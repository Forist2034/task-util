use chrono::{Datelike, Timelike};

pub(super) trait ToICal {
    fn write_ical(&self, buf: &mut impl std::fmt::Write) -> std::fmt::Result;
}
macro_rules! ical_fmt {
    ($t:ty) => {
        impl ToICal for $t {
            fn write_ical(&self, buf: &mut impl std::fmt::Write) -> std::fmt::Result {
                write!(buf, "{self}")
            }
        }
    };
}
ical_fmt!(u8);
ical_fmt!(u16);
ical_fmt!(u32);
ical_fmt!(u64);
ical_fmt!(usize);
impl ToICal for bool {
    fn write_ical(&self, buf: &mut impl std::fmt::Write) -> std::fmt::Result {
        buf.write_str(if *self { "true" } else { "false" })
    }
}
impl ToICal for str {
    fn write_ical(&self, buf: &mut impl std::fmt::Write) -> std::fmt::Result {
        for c in self.chars() {
            match c {
                '\\' => buf.write_str(r"\\")?,
                ';' => buf.write_str(r"\;")?,
                ',' => buf.write_str(r"\,")?,
                '\n' => buf.write_str(r"\n")?,
                _ => buf.write_char(c)?,
            }
        }
        Ok(())
    }
}
impl ToICal for chrono::DateTime<chrono::Utc> {
    fn write_ical(&self, buf: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(
            buf,
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            self.year_ce().1,
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}
impl ToICal for uuid::Uuid {
    fn write_ical(&self, buf: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(buf, "{self}")
    }
}
impl<T: ToICal + ?Sized> ToICal for &T {
    fn write_ical(&self, buf: &mut impl std::fmt::Write) -> std::fmt::Result {
        T::write_ical(self, buf)
    }
}

pub(super) struct ICalTodoBuilder {
    line_buf: String,
    buf: String,
}
impl ICalTodoBuilder {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            line_buf: String::new(),
        }
    }
    fn flush_line(&mut self) {
        let mut line_buf = self.line_buf.as_str();
        loop {
            let (line, tail) = line_buf.split_at(line_buf.floor_char_boundary(75));
            self.buf.push_str(line);
            if tail.is_empty() {
                self.buf.push_str("\r\n");
                break;
            } else {
                self.buf.push_str("\r\n ");
                line_buf = tail;
            }
        }
        self.line_buf.clear();
    }
    pub fn add_task(&mut self, idx: usize, task: &crate::types::task::Task) {
        if let Some((head, tail)) = task.tags.split_first() {
            self.line_buf.push_str("CATEGORIES:");
            let _ = head.name.write_ical(&mut self.line_buf);
            for t in tail {
                self.line_buf.push(',');
                let _ = t.name.write_ical(&mut self.line_buf);
            }
            self.flush_line();
        }

        self.add_simple_prop(
            "DESCRIPTION",
            task.description.as_ref().map_or("", |v| v.as_str()),
        );
        self.add_simple_prop("DTSTAMP", task.created.to_utc());
        self.add_simple_prop("PRIORITY", 0u8);

        if let Some(ref c) = task.completed {
            self.add_simple_prop("COMPLETED", c.to_utc());
            self.add_simple_prop("PERCENT-COMPLETE", "100");
        }
        self.add_simple_prop(
            "STATUS",
            match task.status {
                crate::types::task::Status::Pending => "NEEDS-ACTION",
                crate::types::task::Status::Completed => "COMPLETED",
            },
        );
        self.add_simple_prop("SUMMARY", task.name.as_str());
        self.add_simple_prop("UID", task.id);
        self.add_simple_prop("X-APPLE-SORT-ORDER", (idx + 1) * 100);
    }
    pub fn add_simple_prop(&mut self, prop: &str, val: impl ToICal) {
        self.line_buf.push_str(prop);
        self.line_buf.push(':');
        let _ = val.write_ical(&mut self.line_buf);
        self.flush_line();
    }
    pub fn clear(&mut self) {
        self.buf.clear();
        self.buf.push_str(concat!(
            "BEGIN:VCALENDAR\n",
            "VERSION:2.0\n",
            "PRODID:-//task-util (https://github.com/Forist2034/task-util)\n",
            "BEGIN:VTODO\n"
        ));
    }
    pub fn finish(&mut self) -> &str {
        self.buf.push_str(concat!("END:VTODO\n", "END:VCALENDAR\n"));
        &self.buf
    }
}
