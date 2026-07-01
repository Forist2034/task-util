const path = require("path");
const fs = require("node:fs");

function readRequest(ipc_dir) {
  const req_dir = path.join(ipc_dir, "req");
  try {
    fs.accessSync(req_dir);
  } catch (e) {
    console.warn(e);
    return null;
  }
  for (const filename of fs.readdirSync(req_dir)) {
    const filepath = path.join(req_dir, filename);
    try {
      const request = JSON.parse(
        fs.readFileSync(filepath, { encoding: "utf8" }),
      );
      fs.unlinkSync(filepath);
      return { response_file: path.join(ipc_dir, "resp", filename), request };
    } catch (e) {
      console.warn(e);
    }
  }
  return null;
}
return readRequest(args[0]);
