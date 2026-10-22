# Differential fixtures for the swimlane model.
#
# Runs the board's real `Activity.Graph.layout/2` and `Activity.Warp.scale/3`
# over seeded random scenarios and writes each scenario's inputs and the
# Elixir outputs as JSON, which `tests/golden.rs` replays through the Rust port.
#
#     IMP_AGENT=/path/to/imp-agent elixir gen.exs <layouts> <warps> <collabs> <lanes> <out.json> [small]
#
# Needs Elixir >= 1.18 (built-in `JSON`). The generator gives every item's
# newest event a status, since the port reads "the newest status known" where
# the Elixir reads the newest event's own, and orders loose events by time,
# since the Elixir breaks ties between equal pixels by input order.

root = Path.join(System.fetch_env!("IMP_AGENT"), "lib/imp_agent/activity")
for file <- ~w(warp.ex graph.ex event.ex change.ex collaboration.ex) do
  Code.require_file(Path.join(root, file))
end

alias ImpAgent.Activity.{Collaboration, Event, Graph, Warp}

# The lane split is private to the LiveView component, which needs Phoenix; lift
# its pure functions out of the source and compile them on their own.
defmodule LanesLoader do
  @names [:split, :event_node, :label, :link_label, :detail, :first_line, :slug, :attributed?]

  # A guarded clause's head is `{:when, _, [call, guard]}`.
  defp name_of({:when, _, [call | _]}), do: name_of(call)
  defp name_of({name, _, _}), do: name

  def load(imp_agent) do
    path = Path.join(imp_agent, "lib/imp_agent_web/components/activity_view.ex")
    {:ok, {:defmodule, _, [_, [do: {:__block__, _, body}]]}} = path |> File.read!() |> Code.string_to_quoted()

    defs =
      for {:defp, meta, [head | rest]} <- body, name_of(head) in @names do
        {:def, meta, [head | rest]}
      end

    Module.create(LanesUnderTest, {:__block__, [], defs}, Macro.Env.location(__ENV__))
  end
end

LanesLoader.load(System.fetch_env!("IMP_AGENT"))

defmodule Gen do
  @kinds ~w(item_created item_claimed item_released item_status note heartbeat run_started run_tool run_thought run_reasoning run_model run_finished commit branch_head pr_opened pr_reviewed pr_merged handoff)a
  @statuses ["open", "claimed", "in_progress", "review", "blocked", "done"]
  @loose_kinds ~w(commit branch_head pr_merged note pr_opened)a
  @labels ["omp", "orchestrator", "feature/very-long-branch-name", "a", "imp-design"]
  @since_us 1_790_812_800_000_000
  @sec 1_000_000

  def since_us, do: @since_us

  def pick(list), do: Enum.at(list, :rand.uniform(length(list)) - 1)
  def int(a, b), do: a + :rand.uniform(b - a + 1) - 1
  def chance(p), do: :rand.uniform() < p

  # Whole seconds keep second precision, so `DateTime.to_iso8601/1` prints no
  # fraction for them, as the ledger's whole-second stamps do.
  def dt(us) do
    if rem(us, @sec) == 0,
      do: DateTime.from_unix!(div(us, @sec), :second),
      else: DateTime.from_unix!(us, :microsecond)
  end

  def event(n, us, kind, item, status, title, summary, agent) do
    %{
      id: "e#{n}",
      at: dt(us),
      kind: kind,
      agent: agent,
      item_id: item,
      summary: summary,
      detail: %{status: status, title: title}
    }
  end

  def scenario(i, small? \\ false) do
    :rand.seed(:exsss, {7, 11, i})
    width = pick([300, 700, 1200, 1400, 1900])
    span_days = pick([1, 3, 7, 30])
    since = @since_us
    now = since + span_days * 86_400 * @sec
    n_items = if small?, do: int(1, 7), else: int(0, 25)
    counter = :counters.new(1, [])
    next = fn -> :counters.add(counter, 1, 1); :counters.get(counter, 1) end

    items =
      for k <- 1..max(n_items, 1)//1, n_items > 0 do
        item = "imp-#{int(1, 400)}-#{k}"
        start = since + int(0, span_days * 86_400) * @sec
        n = if small?, do: int(1, 4), else: int(1, 8)

        {evs, _} =
          Enum.map_reduce(1..n, start, fn j, t ->
            gap =
              case int(1, 5) do
                1 -> 0
                2 -> int(1, 90) * @sec
                3 -> int(60, 3_600) * @sec
                4 -> int(3_600, 3 * 3_600) * @sec
                5 -> int(3 * 3_600, 40 * 3_600) * @sec
              end

            t = if j == 1, do: t, else: t + gap + if(chance(0.2), do: int(0, 999_999), else: 0)
            t = min(t, now)
            {{t, j}, t}
          end)

        evs
        |> Enum.map(fn {t, _j} ->
          status = if chance(0.5), do: pick(@statuses), else: nil
          title = if chance(0.3), do: "title #{int(1, 9)}", else: nil
          summary = if chance(0.4), do: "did a thing #{int(1, 99)}\nsecond line", else: nil
          agent = pick(["worker", "omp", "clara", nil, ""])
          event(next.(), t, pick(@kinds), item, status, title, summary, agent)
        end)
        |> Enum.sort_by(&DateTime.to_unix(&1.at, :microsecond))
        |> then(fn list ->
          # The Elixir draws the newest event's own status; give it one, so the
          # port's "newest known status" reads the same.
          List.update_at(list, -1, fn e ->
            put_in(e, [:detail, :status], e.detail.status || pick(@statuses))
          end)
        end)
      end
      |> List.flatten()

    loose =
      for _ <- 1..(if small?, do: int(0, 6), else: int(0, 30))//1 do
        t = since + int(0, span_days * 86_400) * @sec + if(chance(0.3), do: int(0, 999_999), else: 0)
        event(next.(), t, pick(@loose_kinds), nil, nil, nil, if(chance(0.3), do: "loose", else: nil), pick(["worker", nil]))
      end
      |> Enum.sort_by(&DateTime.to_unix(&1.at, :microsecond))

    # Some item-less events travel in `events`, the rest in `loose:`.
    {in_events, in_loose} = Enum.split(loose, div(length(loose), 2))
    events = items ++ in_events

    item_ids = items |> Enum.map(& &1.item_id) |> Enum.uniq()

    lanes =
      if item_ids != [] and chance(0.6) do
        for n <- 1..(if small?, do: int(1, 3), else: int(1, 6)) do
          item = pick(item_ids)
          label = pick(@labels)
          a = since + int(0, span_days * 86_400) * @sec
          b = since + int(0, span_days * 86_400) * @sec

          %{
            id: "#{item}/collab/#{label}-#{n}",
            item_id: item,
            kind: pick([:path, :branch, :handoff, :collab]),
            label: label,
            from: dt(a),
            to: dt(b),
            chosen: pick([nil, true, false]),
            parent: nil,
            agent: label
          }
        end
      else
        []
      end

    edges =
      Enum.flat_map(lanes, fn lane ->
        f = if chance(0.8), do: [%{kind: :fork, lane: lane.id, item_id: lane.item_id, at: lane.from}], else: []
        m = if chance(0.6), do: [%{kind: :merge, lane: lane.id, item_id: lane.item_id, at: lane.to}], else: []
        f ++ m
      end)

    edges =
      if lanes != [] and chance(0.3) do
        [%{kind: :fork, lane: "nobody/lane", item_id: hd(lanes).item_id, at: dt(since)} | edges]
      else
        edges
      end

    limit = pick([nil, nil, 5, 20, 1])

    loose_open =
      pick([false, false, true, [:commit], [:branch_head, :note], [:pr_merged, :commit]])

    opts = [
      width: width,
      since: dt(since),
      now: dt(now),
      limit: limit,
      loose: in_loose,
      loose_open: loose_open,
      tree: %{lanes: lanes, edges: edges}
    ]

    graph = Graph.layout(events, opts)

    %{
      name: "seed-#{i}",
      width: width,
      since: since,
      now: now,
      limit: limit,
      loose_open:
        case loose_open do
          true -> "all"
          false -> "none"
          kinds -> Enum.map(kinds, &to_string/1)
        end,
      events: Enum.map(events, &ev_json/1),
      loose: Enum.map(in_loose, &ev_json/1),
      tree: %{
        lanes:
          Enum.map(lanes, fn l ->
            %{l | kind: to_string(l.kind), from: us(l.from), to: us(l.to)}
          end),
        edges:
          Enum.map(edges, fn e -> %{e | kind: to_string(e.kind), at: us(e.at)} end)
      },
      expected: graph_json(graph)
    }
  end

  def us(%DateTime{} = d), do: DateTime.to_unix(d, :microsecond)

  def ev_json(e) do
    %{
      id: e.id,
      at: us(e.at),
      kind: to_string(e.kind),
      agent: e.agent,
      item_id: e.item_id,
      summary: e.summary,
      status: e.detail.status,
      title: e.detail.title
    }
  end

  def dot_json(d) do
    %{
      id: d.id, at: us(d.at), kind: to_string(d.kind), agent: d.agent, tone: to_string(d.tone),
      title: d.title, count: d.count, x: d.x
    }
  end

  def graph_json(g) do
    %{
      width: g.width, height: g.height, lanes: g.lanes, rows: g.rows,
      total_items: g.total_items, hidden: g.hidden, axis_y: g.axis_y, loose_y: g.loose_y,
      items:
        Enum.map(g.items, fn i ->
          %{
            id: i.id, title: i.title, status: i.status, tone: to_string(i.tone), done: i.done?,
            lane: i.lane, y: i.y, x0: i.x0, x1: i.x1, label_x: i.label_x,
            start_label_x: i.start_label_x, last: us(i.last),
            dots: Enum.map(i.dots, &dot_json/1),
            forks:
              Enum.map(i.forks, fn f ->
                %{
                  id: f.id, kind: to_string(f.kind), label: f.label, title: f.title,
                  chosen: f.chosen, dead: f.dead?, y: f.y, x0: f.x0, x1: f.x1
                }
              end),
            edges:
              Enum.map(i.edges, fn e -> %{kind: to_string(e.kind), dead: e.dead?, d: e.d} end)
          }
        end),
      strip:
        Enum.map(g.strip, fn r ->
          %{kind: to_string(r.kind), count: r.count, open: r.open?, y0: r.y0, cells: r.cells}
        end),
      loose:
        Enum.map(g.loose, fn r ->
          %{kind: to_string(r.kind), count: r.count, y: r.y, dots: Enum.map(r.dots, &dot_json/1)}
        end),
      ticks: Enum.map(g.ticks, fn t -> %{x: t.x, label: t.label, kind: to_string(t.kind)} end),
      gaps: g.gaps
    }
  end



  # -- lanes -------------------------------------------------------------------------

  def lanes_scenario(i, small? \\ false) do
    :rand.seed(:exsss, {29, 31, i})
    n = if small?, do: int(3, 8), else: int(3, 30)

    events =
      for k <- 1..n do
        branch = pick([nil, nil, "fix/imp-7-thing", "feat/x"])
        commit = pick([nil, nil, "0123456789abcdef0123", "abc"])
        kind = if branch && chance(0.5), do: :branch_head, else: pick(@kinds)

        Event.new(
          source: pick([:ledger, :run, :git, :github]),
          seq: k,
          id: pick(["ledger:#{k}", "git:abc def #{k}", "x/y#{k}", "run-1:#{k}"]),
          at: dt(@since_us + int(0, 100_000) * @sec + jitter()),
          kind: kind,
          agent: pick(["worker", "omp", "Zed", "a b", "z/9", nil, "", "  "]),
          item_id: pick([nil, "imp-7", "imp-12"]),
          branch: branch,
          pr: pick([nil, nil, 7, 12]),
          commit: commit,
          summary: pick([nil, "", "one line", "first\nsecond", "  \n lead", "tab\tsep  ed"])
        )
      end

    {lanes, unattributed} = LanesUnderTest.split(events)

    lane_json = fn lane ->
      %{
        id: lane.id,
        label: lane.label,
        nodes: Enum.map(lane.nodes, fn nd -> %{nd | kind: to_string(nd.kind)} end)
      }
    end

    %{
      name: "lanes-#{i}",
      events:
        Enum.map(events, fn e ->
          %{
            id: e.id, at: us(e.at), seq: e.seq, source: to_string(e.source), kind: to_string(e.kind),
            agent: e.agent, item_id: e.item_id, branch: e.branch, pr: e.pr, commit: e.commit,
            summary: e.summary
          }
        end),
      expected: %{lanes: Enum.map(lanes, lane_json), unattributed: Enum.map(unattributed, lane_json)}
    }
  end

  # -- collaboration ---------------------------------------------------------------

  @now_us 1_791_223_200_000_000

  def collab_scenario(i, small? \\ false) do
    :rand.seed(:exsss, {19, 23, i})
    agents = ["a", "b", "c", "ThemeLand", "imp-design", "", "  ", nil]
    named = ["a", "b", "c", "ThemeLand", "imp-design"]
    items = if small?, do: ["imp-1", "imp-2", "imp-3", "imp-12"], else: ["imp-1", "imp-2", "imp-3", "imp-4", "imp-5", "imp-12"]
    shas = ["s1", "s2", "s3", "s4"]
    n = if small?, do: int(10, 20), else: int(4, 40)

    raw =
      for k <- 1..n do
        t = @now_us - int(0, 8 * 86_400) * @sec
        item = if chance(0.12), do: nil, else: pick(items)

        case int(1, 100) do
          x when x <= 36 ->
            {k, :ledger, t + jitter(), :note, pick(agents), item, "note #{k}", %{}}

          x when x <= 46 ->
            {k, :ledger, t, :item_claimed, pick(agents), item, nil, %{status: "claimed"}}

          x when x <= 53 ->
            {k, :ledger, t, :item_created, pick(agents), item, nil, %{status: "open", title: "t #{k}"}}

          x when x <= 57 ->
            {k, :ledger, t, :item_released, pick(agents), item, nil, %{}}

          x when x <= 65 ->
            to = pick(agents)
            {k, :ledger, t, :handoff, to, item, nil, %{from: pick(named), status: "claimed"}}

          x when x <= 73 ->
            {k, :github, t, :pr_reviewed, pick(agents), item, nil, %{}}

          x when x <= 80 ->
            commits = Enum.take_random(shas, int(0, 3))
            {k, :git, t, :pr_merged, nil, item, nil, %{commits: commits, parents: ["a", "b"]}}

          x when x <= 90 ->
            {k, :git, t + jitter(), :commit, pick([nil, nil, "a", ""]), if(chance(0.5), do: nil, else: item), nil, %{sha: pick(shas)}}

          _ ->
            {k, :ledger, t, :item_status, pick(agents), item, nil, %{status: pick(["claimed", "in_progress", "done", "review", nil])}}
        end
      end

    events =
      raw
      |> Enum.map(fn {k, source, t, kind, agent, item, summary, detail} ->
        sha = Map.get(detail, :sha)
        detail = Map.delete(detail, :sha)
        branch = if item && chance(0.4), do: "feat/#{item}", else: nil

        Event.new(
          source: source,
          seq: k,
          at: dt(t),
          kind: kind,
          agent: agent,
          item_id: item,
          branch: branch,
          pr: if(kind == :pr_merged, do: k, else: nil),
          commit: sha,
          summary: summary,
          detail: detail
        )
      end)
      |> Event.sort()

    item_map =
      for item <- items, chance(0.8), into: %{} do
        entry =
          %{}
          |> then(&if(chance(0.85), do: Map.put(&1, :status, pick(["claimed", "claimed", "in_progress", "done", "review", nil])), else: &1))
          |> then(&if(chance(0.5), do: Map.put(&1, :assignee, pick(named ++ [nil])), else: &1))
          |> then(&if(chance(0.3), do: Map.put(&1, :title, "title of #{item}"), else: &1))

        {item, entry}
      end

    result = Collaboration.build(events, item_map, dt(@now_us))

    %{
      name: "collab-#{i}",
      now: @now_us,
      events:
        Enum.map(events, fn e ->
          %{
            id: e.id, at: us(e.at), seq: e.seq, source: to_string(e.source), kind: to_string(e.kind),
            agent: e.agent, item_id: e.item_id, branch: e.branch, pr: e.pr, commit: e.commit,
            summary: e.summary, status: Map.get(e.detail, :status), title: Map.get(e.detail, :title),
            from: Map.get(e.detail, :from), commits: Map.get(e.detail, :commits, [])
          }
        end),
      items:
        Map.new(item_map, fn {id, entry} ->
          {id,
           Map.new(entry, fn
             {:title, v} -> {"title", v}
             {key, v} -> {to_string(key), %{v: v}}
           end)}
        end),
      expected: %{
        pairs:
          Enum.map(result.pairs, fn p ->
            %{p | handoffs: Enum.map(p.handoffs, fn h -> %{h | at: us(h.at)} end)}
          end),
        threads:
          Enum.map(result.threads, fn t ->
            %{
              item_id: t.item_id, title: t.title, status: t.status && to_string(t.status),
              steps:
                Enum.map(t.steps, fn s -> %{at: us(s.at), kind: to_string(s.kind), agent: s.agent, item_id: s.item_id, label: s.label} end),
              agents: t.agents, stale: t.stale?, stale_agent: t.stale_agent,
              stale_since: t.stale_since && us(t.stale_since)
            }
          end)
      }
    }
  end

  def jitter, do: if(chance(0.2), do: int(0, 999_999), else: 0)

  def warp_scenario(i, small? \\ false) do
    :rand.seed(:exsss, {13, 17, i})
    width = pick([200, 300, 600, 1400, 2400])
    left = pick([0, 0, 50, 59.95])
    right = pick([0, 0, 60, 59.95])
    span_days = pick([1, 3, 10, 60])
    since = @since_us
    n = if small?, do: int(0, 14), else: int(0, 60)

    times =
      for _ <- 1..n//1, n > 0 do
        base = since + int(0, span_days * 86_400) * @sec
        # bursts: some events cluster around earlier ones
        base + if(chance(0.5), do: int(0, 7_200) * @sec, else: 0) + if(chance(0.3), do: int(0, 999_999), else: 0)
      end

    bounds_since = if chance(0.5), do: since, else: nil
    bounds_now = if chance(0.5), do: since + span_days * 86_400 * @sec, else: nil

    events = Enum.map(times, fn t -> %{at: dt(t)} end)

    opts =
      [left: left, right: right] ++
        if(bounds_since, do: [since: dt(bounds_since)], else: []) ++
        if(bounds_now, do: [now: dt(bounds_now)], else: [])

    axis = Warp.scale(events, width, opts)

    probes =
      (Enum.take(times, 12) ++ [since - 3600 * @sec, since + span_days * 86_400 * @sec + 3600 * @sec, since + div(span_days * 86_400, 2) * @sec] ++
         Enum.map(0..8, fn k -> since + div(span_days * 86_400 * @sec * k, 8) end))
      |> Enum.uniq()

    %{
      name: "warp-#{i}",
      width: width,
      left: left,
      right: right,
      since: bounds_since,
      now: bounds_now,
      times: times,
      probes: probes,
      xs: Enum.map(probes, fn t -> axis.x.(dt(t)) end),
      ticks: Enum.map(axis.ticks, fn t -> %{x: t.x, label: t.label, kind: to_string(t.kind)} end),
      gaps: axis.gaps
    }
  end
end

[count, wcount, ccount, lcount, path | rest] = System.argv()
small? = rest == ["small"]
count = String.to_integer(count)
wcount = String.to_integer(wcount)
ccount = String.to_integer(ccount)
lcount = String.to_integer(lcount)

out = %{
  layouts: Enum.map(1..count//1, &Gen.scenario(&1, small?)),
  warps: Enum.map(1..wcount//1, &Gen.warp_scenario(&1, small?)),
  collabs: Enum.map(1..ccount//1, &Gen.collab_scenario(&1, small?)),
  lanes: Enum.map(1..lcount//1, &Gen.lanes_scenario(&1, small?))
}

File.write!(path, JSON.encode!(out))
IO.puts("wrote #{count} layouts, #{wcount} warps, #{ccount} collaborations and #{lcount} lane splits to #{path}")
