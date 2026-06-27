#!/usr/bin/env bash
# Returns the single highest-priority unrefined issue (with body) as JSON.
gh issue list --state open \
  --label "status:unrefined" \
  --json number,title,labels,body,createdAt \
  --limit 50 \
  --jq '
    map(. + {prank: ({"priority:critical":0,"priority:high":1,"priority:medium":2,"priority:low":3}
      [([.labels[].name | select(startswith("priority:"))] | first) // "priority:medium"] // 2)})
    | sort_by(.prank, .createdAt)
    | first'
