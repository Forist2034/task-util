{
  project = {
    Info = {
      id | String,
      name | String,
      root | String,
      description | String | optional,
      created | String,
      color | String,
      data | optional,
      external_tools | optional
    }
  },
  task = {
    Info = {
      id | String,
      name | String,
      status | optional,
      description | String | optional,
      created | String,
      completed | String | optional,
      tags | Array { id | String, name | String, .. } | optional,
      data | optional,
      external_tools | optional
    },
    mk_task | Dyn -> Info -> Info = fun state info => (std.record.get_or info.id {} state) & info
  }
}
