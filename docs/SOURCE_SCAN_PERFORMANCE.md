# Telegram source discovery performance

Measured on 2026-10-09 with Samsung SM-A256E (Android 14), the connected
account's 636 Main/Archive chats, and the app's existing TDLib session. Each
run was a manual **Resync**. The first two runs used an Audio search limit of
1; the third used 50. All runs used 8 concurrent source requests and a
Document search limit of 50.

| Measure | Audio limit 1, run 1 | Audio limit 1, run 2 | Audio limit 50 |
|---|---:|---:|---:|
| Scan duration | 80 s | 83 s | 20 s |
| Chats checked | 636 | 636 | 636 |
| Total requests | 2,296 | 2,298 | 1,976 |
| Audio requests | 963 | 963 | 643 |
| Empty Audio pages with another cursor | 327 inferred | 327 | 7 |
| Most Audio pages for one chat | not recorded | 199 | 5 |
| Slowest request | 19.4 s | 18.4 s | 0.4 s |
| Audio / Document sources found | 123 / 14 | 123 / 14 | 123 / 14 |
| Flood waits / timeouts | 0 / 0 | 0 / 0 | 0 / 0 |

The scan listed chats in about 1.1–1.3 seconds. With a one-item Audio limit,
two runs checked 633–635 chats within 20–30 seconds but spent the remaining
time on the last few. TDLib returned empty Audio result pages with a nonzero
`next_from_message_id`; the app correctly followed that cursor but requested
only one item per page. In the second run, 327 Audio pages had another cursor,
and one chat alone needed 199 Audio requests. The repeated filtered searches
also produced individual waits of 14–19 seconds. A larger page size reduced
the empty-page chain and eliminated the long tail in this device run.

The fix is `AUDIO_SEARCH_LIMIT = 50` in
`crates/music_app/src/adapters/telegram/sources.rs`. The app still follows
TDLib's cursor, checks Documents after an exhausted Audio search, and retains
the same source classification. One run on one account and network does not
establish a general performance guarantee. The remaining 20-second scan made
884 Document requests and examined 7,866 Document messages; that is the main
remaining work if further speedup is needed.

The source engine writes aggregate `TuneStashScan` entries to Android logcat:

```sh
adb logcat -s TuneStashScan:I '*:S'
```

`start`, `chats_loaded`, 10-second `progress`, `complete`/`failed`, and
`slow_request` entries report elapsed time, queue depth, request latency,
Audio/Document page counts, checked chats, timeouts, and flood waits. Logs
exclude account and chat IDs, names, message data, and Telegram error text.
