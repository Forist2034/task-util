const os = require("node:os");
const path = require("node:path");

const ipc_dir = path.join(
  (typeof process != "undefined" && process.env && process.env.XDG_DATA_HOME) ||
    path.join(os.homedir(), ".local", "share"),
  "task-util",
  "super-productivity-ipc",
);
return { ipc_dir };
