import type { PluginAPI, Task } from "@super-productivity/plugin-api";
import getIpcPath from "./getIpcPath.js" with { type: "text" };
import readRequest from "./readRequest.js" with { type: "text" };
import writeResponse from "./writeResponse.js" with { type: "text" };

declare const PluginAPI: PluginAPI;

async function handle_message(msg: any) {
  const val = msg.args;
  switch (msg.op) {
    case "add_task": {
      let ret = await PluginAPI.addTask(val);
      return { task_id: ret };
    }
    case "start_task": {
      // currentTimestamp is how super-productivity tracks started task internally
      const v: any = { currentTimestamp: val.start_time };
      await PluginAPI.updateTask(val.id, v);
      return null;
    }
    case "stop_task": {
      const v: any = { currentTimestamp: null };
      await PluginAPI.updateTask(val.id, v);
      return null;
    }
    case "finish_task": {
      const v: Partial<Task> | { currentTimestamp: null } = {
        currentTimestamp: null,
        isDone: true,
        doneOn: val.completed_time,
      };
      const vAny: any = v;
      await PluginAPI.updateTask(val.id, vAny);
      return null;
    }
    case "update_task": {
      await PluginAPI.updateTask(val.id, val.updates);
      return null;
    }
    case "add_project": {
      let ret = await PluginAPI.addProject(val);
      return { project_id: ret };
    }
    case "update_project": {
      await PluginAPI.updateProject(val.id, val.updates);
      return null;
    }
    case "add_tag": {
      let ret = await PluginAPI.addTag(val);
      return { tag_id: ret };
    }
    case "update_tag": {
      await PluginAPI.updateTag(val.id, val.updates);
      return null;
    }
    default:
      throw "Unknown message";
  }
}

let timer = null;

async function runNodeScript(script: string, args: any[]): Promise<any> {
  let ret = await PluginAPI.executeNodeScript!!({ script, args });
  if (!ret.success) {
    console.error(ret.error);
    throw ret.error;
  } else {
    return ret.result!!;
  }
}
async function poll_request(ipc_dir: string) {
  const req = await runNodeScript(readRequest, [ipc_dir]);
  if (req == null) {
    return;
  }
  try {
    const resp = await handle_message(req.request);
    await runNodeScript(writeResponse, [
      req.response_file,
      JSON.stringify({ status: "ok", data: resp }),
    ]);
  } catch (e: any) {
    await runNodeScript(writeResponse, [
      req.response_file,
      JSON.stringify({ status: "err", data: e.toString() }),
    ]);
  }
}
async function init() {
  const ipc_dir: any = await runNodeScript(getIpcPath, []);
  console.log(ipc_dir);
  timer = setInterval(() => poll_request(ipc_dir.ipc_dir), 1000);
}
setTimeout(init, 500);
