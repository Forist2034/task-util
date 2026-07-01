import net from "node:net";
import os from "node:os";
import type { PluginAPI, Task } from "@super-productivity/plugin-api";
import path from "node:path";
import { SmartBuffer } from "smart-buffer";

declare const PluginAPI: PluginAPI;
declare const nodeScripts: {
  getIpcPath: String;
  readRequest: String;
};

async function handle_message(msg: any) {
  const val = msg.value;
  switch (msg.type) {
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
    // FIXME: stop task from plugin
    case "stop_task": {
      const v: any = { currentTimestamp: null };
      await PluginAPI.updateTask(val.id, v);
      return null;
    }
    // not work
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

const server = net.createServer((c) => {
  let size: number | null;
  let mainBuf = new SmartBuffer();
  let otherBuf = new SmartBuffer();
  c.on("data", (chunk: Buffer) => {
    mainBuf.writeBuffer(chunk);
    if (size == null) {
      size = mainBuf.readUInt32LE();
    } else if (mainBuf.remaining() >= size) {
      const val = mainBuf.readString(size, "utf8");
      handle_message(JSON.parse(val)).then(
        (val) => {
          c.write(JSON.stringify({ state: "ok", val }));
        },
        (e) => {
          c.write(JSON.stringify({ state: "err", val: e }));
        },
      );

      size = null;

      otherBuf.clear();
      otherBuf.writeBuffer(mainBuf.readBuffer());
      let buf = otherBuf;
      otherBuf = mainBuf;
      mainBuf = buf;
    }
  });
});
server.listen(path.join(os.homedir(), "super-productivity.sock"));
