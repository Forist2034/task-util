let state = import "{file}.json" in
{{
  project =
    state.project
    & {{
      id = "{id}",
      root = "{root}",
      name = "{name}",
      created = "{created}",
      color = "{color}"
    }},
  tasks =
    std.array.map
      (fun t => std.record.get_or t.id {{}} state.tasks & t)
      []
}}
