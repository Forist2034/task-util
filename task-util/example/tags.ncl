let state = import "tags.json" in
{
  sample =
    (std.record.get_or "02c505c5-409d-4228-ac4c-beebf744e866" {} state)
    & {
      id = "02c505c5-409d-4228-ac4c-beebf744e866",
      name = "tag0",
      created = "2026-06-29T07:24:30+08:00",
      color = "#ffffff",
      external_tools = {}
    },
  all_tags = [sample]
}
