import type { GitLogEntry } from "@/lib/types"

const COLORS = [
  "#3b82f6",
  "#a855f7",
  "#10b981",
  "#f97316",
  "#ec4899",
  "#06b6d4",
]
export interface GraphRow {
  lane: number
  color: string
  before: { hash: string; color: string }[]
  after: { hash: string; color: string }[]
  parents: string[]
}

// Retain pending parent lanes across the whole loaded prefix. Pagination appends
// rows without changing earlier lane assignments. Missing (filtered) parents
// remain pending; they are never connected to a different visible commit.
export function buildHistoryGraph(
  entries: Pick<GitLogEntry, "full_hash" | "parents">[],
  filtered = false
): GraphRow[] {
  const lanes: { hash: string; color: string }[] = []
  let nextColor = 0
  const visible = new Set(entries.map((entry) => entry.full_hash))
  const last = entries[entries.length - 1]?.full_hash
  return entries.map((entry) => {
    let lane = lanes.findIndex((l) => l.hash === entry.full_hash)
    if (lane < 0) {
      lane = lanes.length
      lanes.push({
        hash: entry.full_hash,
        color: COLORS[nextColor++ % COLORS.length],
      })
    }
    const before = [...lanes]
    const color = lanes[lane].color
    lanes.splice(lane, 1)
    const parents = (entry.parents ?? []).filter(
      (parent) => !filtered || visible.has(parent) || entry.full_hash === last
    )
    for (const [i, parent] of parents.entries()) {
      if (!lanes.some((l) => l.hash === parent)) {
        lanes.splice(Math.min(lane + i, lanes.length), 0, {
          hash: parent,
          color: i === 0 ? color : COLORS[nextColor++ % COLORS.length],
        })
      }
    }
    return {
      lane,
      color,
      before,
      after: [...lanes],
      parents,
    }
  })
}

export function HistoryGraphRail({
  row,
  width,
  head,
}: {
  row: GraphRow
  width: number
  head: boolean
}) {
  const x = (lane: number) => 9 + lane * 12
  return (
    <div
      className="relative shrink-0 overflow-hidden"
      style={{ width }}
      aria-hidden
    >
      <svg className="absolute inset-0 size-full" preserveAspectRatio="none">
        {row.after.map((lane, i) => (
          <line
            key={lane.hash}
            x1={x(i)}
            x2={x(i)}
            y1={40}
            y2="100%"
            stroke={lane.color}
            strokeWidth={1.5}
          />
        ))}
      </svg>
      <svg className="absolute top-0 left-0" width={width} height={40}>
        {row.before.map((lane, i) => {
          if (i === row.lane)
            return (
              <path
                key={lane.hash}
                d={`M${x(i)} 0 V20`}
                stroke={lane.color}
                fill="none"
                strokeWidth={1.5}
              />
            )
          const target = row.after.findIndex((l) => l.hash === lane.hash)
          return (
            <path
              key={lane.hash}
              d={`M${x(i)} 0 V12 Q${x(i)} 20 ${x(target)} 28 V40`}
              stroke={lane.color}
              fill="none"
              strokeWidth={1.5}
            />
          )
        })}
        {row.parents.map((parent) => {
          const target = row.after.findIndex((l) => l.hash === parent)
          return (
            <path
              key={parent}
              d={`M${x(row.lane)} 20 Q${x(target)} 20 ${x(target)} 30 V40`}
              stroke={row.after[target]?.color ?? row.color}
              fill="none"
              strokeWidth={1.5}
            />
          )
        })}
        <circle cx={x(row.lane)} cy={20} r={head ? 5 : 3.5} fill={row.color} />
        {head && (
          <circle cx={x(row.lane)} cy={20} r={2} className="fill-background" />
        )}
      </svg>
    </div>
  )
}
