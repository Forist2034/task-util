use std::{
    fmt::Debug,
    io::{Read, Write},
    os::fd::{AsFd, AsRawFd, OwnedFd},
};

use anyhow::Context;
use hyper::body::Body;
use rustix::fs::{AtFlags, MemfdFlags, Mode, OFlags};

#[derive(Debug, serde::Deserialize)]
struct ApiError {
    code: String,
    message: String,
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "api error: {}: {}", self.code, self.message)
    }
}
impl std::error::Error for ApiError {}

#[derive(Debug, serde::Deserialize)]
struct ApiResult<T> {
    ok: bool,
    #[serde(default = "Option::default")]
    data: Option<T>,
    #[serde(default)]
    error: Option<ApiError>,
}
impl<T: Debug> ApiResult<T> {
    fn into_result(self) -> anyhow::Result<T> {
        match self {
            Self {
                ok: true,
                data: Some(d),
                error: None,
            } => Ok(d),
            Self {
                ok: false,
                data: None,
                error: Some(e),
            } => Err(anyhow::Error::new(e).context("api returns error")),
            _ => anyhow::bail!("invalid api result: {self:?}"),
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "status", content = "data")]
enum IpcResult<T> {
    Ok(T),
    Err(String),
}
impl<T> IpcResult<T> {
    fn into_result(self) -> anyhow::Result<T> {
        match self {
            Self::Ok(r) => Ok(r),
            Self::Err(e) => Err(anyhow::Error::msg(e)),
        }
    }
}

#[derive(serde::Serialize)]
#[serde(transparent)]
struct SpTimestamp(i64);
impl From<chrono::DateTime<chrono::FixedOffset>> for SpTimestamp {
    fn from(value: chrono::DateTime<chrono::FixedOffset>) -> Self {
        Self(value.timestamp_millis())
    }
}

#[derive(serde::Serialize)]
struct SpProject<'a> {
    title: &'a str,
    created: SpTimestamp,
}
impl<'a> From<&'a crate::types::project::ProjectInfo> for SpProject<'a> {
    fn from(value: &'a crate::types::project::ProjectInfo) -> Self {
        Self {
            title: &value.name,
            created: value.created.into(),
        }
    }
}

#[derive(serde::Serialize)]
struct SpTag<'a> {
    title: &'a str,
    color: &'a str,
    created: SpTimestamp,
}
impl<'a> From<&'a crate::types::tag::TagInfo> for SpTag<'a> {
    fn from(value: &'a crate::types::tag::TagInfo) -> Self {
        Self {
            title: &value.name,
            color: &value.color,
            created: value.created.into(),
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SpTask<'a> {
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<&'a str>,
    is_done: bool,
    project_id: &'a str,
    tag_ids: Vec<&'a str>,
    created: SpTimestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    done_on: Option<SpTimestamp>,
}
impl<'a> SpTask<'a> {
    fn from_task(
        project: &'a crate::types::project::ProjectInfo,
        task: &'a crate::types::task::Task,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            title: &task.name,
            notes: task.description.as_ref().map(String::as_str),
            is_done: task.status == crate::types::task::Status::Completed,
            project_id: match &project.external_tools.super_productivity {
                Some(crate::types::project::SuperProductivity { id: Some(i), .. }) => i.as_str(),
                _ => anyhow::bail!("project is not added to super-productivity"),
            },
            tag_ids: {
                let mut ret = Vec::with_capacity(task.tags.len());
                for t in task.tags.iter() {
                    match &t.external_tools.super_productivity {
                        Some(crate::types::tag::SuperProductivity { id: Some(i), .. }) => {
                            ret.push(i.as_str())
                        }
                        _ => anyhow::bail!(
                            "tag {:?} ({}) is not added to super-productivity",
                            t.name,
                            t.id
                        ),
                    }
                }
                ret
            },
            created: task.created.into(),
            done_on: task.completed.map(Into::into),
        })
    }
}

#[derive(serde::Serialize)]
#[serde(tag = "op", content = "args")]
#[serde(rename_all = "snake_case")]
enum IpcReq<'a> {
    AddTask(SpTask<'a>),
    UpdateTask {
        id: &'a str,
        updates: SpTask<'a>,
    },
    // StartTask {
    //     id: &'a str,
    //     start_time: SpTimestamp,
    // },
    // StopTask {
    //     id: &'a str,
    // },
    FinishTask {
        id: &'a str,
        completed_time: SpTimestamp,
    },
    AddProject(SpProject<'a>),
    UpdateProject {
        id: &'a str,
        updates: SpProject<'a>,
    },
    AddTag(SpTag<'a>),
    UpdateTag {
        id: &'a str,
        updates: SpTag<'a>,
    },
}

fn get_task_id<'a>(task: &'a crate::types::task::Task) -> anyhow::Result<&'a str> {
    match &task.external_tools.super_productivity {
        Some(crate::types::task::SuperProductivity { id: Some(tid) }) => Ok(tid.as_str()),
        _ => anyhow::bail!("task is not added to super-productivity"),
    }
}

pub struct SuperProductivity {
    rt: tokio::runtime::Handle,
    ipc_req: OwnedFd,
    ipc_resp: OwnedFd,
    pid: rustix::process::Pid,
    fd_path_buf: Vec<u8>,
    body_buf: Vec<u8>,
}
impl SuperProductivity {
    pub fn new(rt: tokio::runtime::Handle) -> anyhow::Result<Self> {
        let mut ipc_path = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let mut ret: std::path::PathBuf = std::env::var_os("HOME")?.into();
                ret.push(".local/share");
                Some(ret)
            })
            .context("can't find ipc path")?;
        ipc_path.push("task-util");
        ipc_path.push("super-productivity-ipc");
        std::fs::create_dir_all(&ipc_path).context("failed to create ipc dir")?;

        ipc_path.push("req");
        std::fs::create_dir_all(&ipc_path).context("failed to create request dir")?;
        let ipc_req = rustix::fs::open(
            &ipc_path,
            OFlags::DIRECTORY | OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .context("failed to open request dir")?;
        ipc_path.pop();

        ipc_path.push("resp");
        std::fs::create_dir_all(&ipc_path).context("failed to create response dir")?;
        let ipc_resp = rustix::fs::open(
            &ipc_path,
            OFlags::DIRECTORY | OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .context("failed to open ipc response dir")?;

        Ok(Self {
            ipc_req,
            ipc_resp,
            rt,
            pid: rustix::process::getpid(),
            fd_path_buf: Vec::new(),
            body_buf: Vec::new(),
        })
    }
    fn call_api<R: Debug + serde::de::DeserializeOwned>(
        &mut self,
        method: http::Method,
        uri: &str,
        body: String,
    ) -> anyhow::Result<R> {
        let (mut http_conn, conn) = self.rt.block_on(async {
            hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(
                tokio::net::TcpStream::connect(std::net::SocketAddrV4::new(
                    std::net::Ipv4Addr::LOCALHOST,
                    3876,
                ))
                .await
                .context("failed to connect to rest server")?,
            ))
            .await
            .context("http handshake failed")
        })?;
        let conn = self.rt.spawn(conn);
        let (_, body) = self
            .rt
            .block_on(
                http_conn.send_request(
                    http::Request::builder()
                        .method(method)
                        .uri(uri)
                        .header(
                            http::header::HOST,
                            const { http::header::HeaderValue::from_static("127.0.0.1:3876") },
                        )
                        .body(body)
                        .unwrap(),
                ),
            )
            .context("failed to send http request")?
            .into_parts();

        self.body_buf.clear();
        let mut body = std::pin::pin!(body);
        while !body.is_end_stream() {
            let f = self
                .rt
                .block_on(std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)))
                .context("failed to read response body")?
                .context("failed to get frame data")?;
            if let Ok(d) = f.into_data() {
                self.body_buf.extend_from_slice(&d);
            }
        }

        let ret = serde_json::from_slice::<ApiResult<R>>(&self.body_buf)
            .context("failed to decode response body")?
            .into_result()
            .context("api returns error");
        self.rt
            .block_on(conn)
            .unwrap()
            .context("connection error")?;

        ret
    }
    fn call_ipc<R: serde::de::DeserializeOwned>(&mut self, req: &IpcReq<'_>) -> anyhow::Result<R> {
        let req_id = uuid::Uuid::new_v4();
        let mut path_buf = [0; uuid::fmt::Hyphenated::LENGTH + 1];
        req_id.as_hyphenated().encode_lower(&mut path_buf);
        let ipc_path = std::ffi::CStr::from_bytes_with_nul(&path_buf).unwrap();

        // if use pipe as request file, super productivity plugin would be blocked
        // when cli program is exited without removing request file
        let req_fd = {
            let mut fd = std::fs::File::from(
                rustix::fs::memfd_create(ipc_path, MemfdFlags::CLOEXEC)
                    .context("failed to create memfd")?,
            );

            self.body_buf.clear();
            serde_json::to_writer(&mut self.body_buf, req).unwrap();
            fd.write_all(&self.body_buf)
                .context("failed to write request")?;

            fd
        };
        rustix::fs::mkfifoat(self.ipc_resp.as_fd(), ipc_path, Mode::RUSR | Mode::WUSR)
            .context("failed to create response pipe")?;

        self.fd_path_buf.clear();
        let _ = write!(
            &mut self.fd_path_buf,
            "/proc/{}/fd/{}\0",
            self.pid,
            req_fd.as_raw_fd()
        );
        rustix::fs::symlinkat(
            std::ffi::CStr::from_bytes_with_nul(&self.fd_path_buf).unwrap(),
            self.ipc_req.as_fd(),
            ipc_path,
        )
        .context("failed to create request symlink")?;

        let mut resp = std::fs::File::from(
            rustix::fs::openat(
                self.ipc_resp.as_fd(),
                ipc_path,
                OFlags::RDONLY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .context("failed to open response file")?,
        );
        self.body_buf.clear();
        resp.read_to_end(&mut self.body_buf)
            .context("failed to read response file")?;

        match rustix::fs::unlinkat(self.ipc_req.as_fd(), ipc_path, AtFlags::empty())
            .and_then(|_| rustix::fs::unlinkat(self.ipc_resp.as_fd(), ipc_path, AtFlags::empty()))
        {
            Ok(()) | Err(rustix::io::Errno::NOENT) => (),
            Err(e) => return Err(anyhow::Error::new(e).context("failed to remove ipc file")),
        }

        serde_json::from_slice::<IpcResult<R>>(&self.body_buf)
            .context("failed to deserialize response")?
            .into_result()
            .context("ipc returned error")
    }

    pub fn add_tag(
        &mut self,
        update: bool,
        tag: &crate::types::tag::TagInfo,
        state: &mut crate::native::TagsHandle,
    ) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct Resp {
            tag_id: String,
        }
        match &tag.external_tools.super_productivity {
            Some(crate::types::tag::SuperProductivity { id: Some(tid) }) => {
                if update {
                    self.call_ipc(&IpcReq::UpdateTag {
                        id: tid.as_str(),
                        updates: tag.into(),
                    })
                } else {
                    Ok(())
                }
            }
            _ => {
                let resp: Resp = self.call_ipc(&IpcReq::AddTag(tag.into()))?;
                state
                    .state
                    .entry(tag.id)
                    .or_default()
                    .external_tools
                    .super_productivity =
                    Some(crate::types::tag::SuperProductivityState { id: resp.tag_id });
                Ok(())
            }
        }
    }
    pub fn add_project(
        &mut self,
        update: bool,
        project: &crate::types::project::ProjectInfo,
        state: &mut crate::native::ProjectHandle,
    ) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct Resp {
            project_id: String,
        }
        match &project.external_tools.super_productivity {
            Some(crate::types::project::SuperProductivity { id: Some(pid) }) => {
                if update {
                    self.call_ipc(&IpcReq::UpdateProject {
                        id: pid.as_str(),
                        updates: project.into(),
                    })
                } else {
                    Ok(())
                }
            }
            _ => {
                let resp: Resp = self.call_ipc(&IpcReq::AddProject(project.into()))?;
                state.state.project.external_tools.super_productivity =
                    Some(crate::types::project::SuperProductivityState {
                        id: resp.project_id,
                    });
                Ok(())
            }
        }
    }
    pub fn add_task(
        &mut self,
        update: bool,
        project: &crate::types::project::ProjectInfo,
        task: &crate::types::task::Task,
        state: &mut crate::native::ProjectHandle,
    ) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct Resp {
            task_id: String,
        }
        let sp_task = SpTask::from_task(project, task)
            .context("failed to convert to super-productivity task")?;
        match &task.external_tools.super_productivity {
            Some(crate::types::task::SuperProductivity { id: Some(tid) }) => {
                if update {
                    self.call_ipc(&IpcReq::UpdateTask {
                        id: tid.as_str(),
                        updates: sp_task,
                    })
                } else {
                    Ok(())
                }
            }
            _ => {
                let resp: Resp = self.call_ipc(&IpcReq::AddTask(sp_task))?;
                state
                    .state
                    .tasks
                    .entry(task.id)
                    .or_default()
                    .external_tools
                    .super_productivity =
                    Some(crate::types::task::SuperProductivityState { id: resp.task_id });
                Ok(())
            }
        }
    }

    pub fn start_task(
        &mut self,
        task: &crate::types::task::Task,
        _: chrono::DateTime<chrono::FixedOffset>,
    ) -> anyhow::Result<()> {
        let _: serde::de::IgnoredAny = self.call_api(
            http::Method::POST,
            &format!("/tasks/{}/start", get_task_id(task)?),
            String::new(),
        )?;
        Ok(())
    }
    pub fn stop_task(&mut self, _: &crate::types::task::Task) -> anyhow::Result<()> {
        let _: serde::de::IgnoredAny =
            self.call_api(http::Method::POST, "/task-control/stop", String::new())?;
        Ok(())
    }
    pub fn finish_task(
        &mut self,
        task: &crate::types::task::Task,
        stop_time: chrono::DateTime<chrono::FixedOffset>,
    ) -> anyhow::Result<()> {
        self.stop_task(task)?;
        self.call_ipc(&IpcReq::FinishTask {
            id: get_task_id(task)?,
            completed_time: stop_time.into(),
        })
    }
    pub fn set_task_complete(
        &mut self,
        task: &crate::types::task::Task,
        stop_time: chrono::DateTime<chrono::FixedOffset>,
    ) -> anyhow::Result<()> {
        self.call_ipc(&IpcReq::FinishTask {
            id: get_task_id(task)?,
            completed_time: stop_time.into(),
        })
    }
}
