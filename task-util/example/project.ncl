let state = import "project.json" in
{
  project =
    state.project
    & {
      id = "c405b50e-2055-4f99-8327-5beede8ba9d1",
      root = "sample",
      name = "proj0",
      created = "2026-06-29T07:24:11+08:00",
      color = "#3c6dff",
      data = {},
      external_tools = {}
    },
  tasks =
    std.array.map
      (fun t => std.record.get_or t.id {} state.tasks & t)
      [
        {
          id = "905a0248-7f87-4a10-9442-3971e74a0e11",
          name = "task0",
          description = m%"
            long description
        "%,
          created = "2026-06-28T21:53:04+08:00",
          tags = [
            (import "tags.ncl").sample
          ],
          data = {},
          external_tools = {
            taskwarrior = {},
            caldav = {},
            super-productivity = {}
          }
        },
        {
          id = "034f4f3f-697d-4eeb-ad62-a2948978b01f",
          name = "task1",
          created = "2026-07-01T12:36:23+08:00"
        }
      ]
}
