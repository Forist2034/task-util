{
  info = {
    task_index | Number,
    done | Bool
  },
  data = {
    type_id = "d74c782c-0434-44d0-a0b1-93ef59d6480f",
    type_name = "sample_data",
    data = info
  },
  external_tools = {
    timewarrior = {
      extra_tags = (if info.done then ["done"] else []) @ ["task_index:%{info.task_index}"]
    }
  }
}
