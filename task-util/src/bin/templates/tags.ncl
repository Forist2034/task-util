let state = import "{file}.json" in
let mk_tag = fun t => (std.record.get_or t.id {{}} state) & t in
{{
  all_tags = []
}}
