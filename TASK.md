| ID | Epic | Task | Priority | Estimate | Dependencies | Acceptance Criteria |
|---|---|---|---|---|---|---|
| TASK-001 | Foundation | ایجاد ساختار Cargo Workspace دو crate | P0 | 1d | - | `music_core` و `music_app` ساخته شده و build می‌شوند |
| TASK-002 | Foundation | تنظیم Rust Toolchain و Cargo Config | P0 | 0.5d | TASK-001 | fmt/check/test روی core موفق است |
| TASK-003 | Foundation | ایجاد CI Pipeline | P1 | 1d | TASK-002 | اجرای خودکار test و lint در PR |
| TASK-101 | Core Architecture | ایجاد Domain Layer | P0 | 3d | TASK-001 | Entityهای اصلی بدون وابستگی platform ساخته شده‌اند |
| TASK-102 | Core Architecture | ایجاد Typed Identifierها | P0 | 1d | TASK-101 | TrackId/SourceId/... مستقل هستند |
| TASK-103 | Core Architecture | ایجاد State Machineهای Core | P0 | 2d | TASK-101 | Auth/Sync/Download/Playback state تست دارند |
| TASK-201 | Application Layer | تعریف Core Ports | P0 | 2d | TASK-101 | Interfaceهای Repository/Media/Telegram ساخته شده |
| TASK-202 | Application Layer | پیاده‌سازی Use Caseها | P0 | 4d | TASK-201 | عملیات اصلی از طریق application layer انجام می‌شوند |
| TASK-301 | Runtime | ایجاد Application Runtime | P0 | 4d | TASK-202 | Runtime مالک سرویس‌های طولانی‌مدت است |
| TASK-302 | Runtime | Command/Event Bus | P0 | 3d | TASK-301 | bounded channel و event model پیاده شده |
| TASK-401 | Database | ایجاد SQLite Adapter | P0 | 3d | TASK-201 | دسترسی DB خارج UI انجام می‌شود |
| TASK-402 | Database | طراحی Schema SQLite | P0 | 2d | TASK-401 | Tables اصلی ایجاد شده‌اند |
| TASK-403 | Database | Migration System | P1 | 2d | TASK-402 | Migration versioning فعال است |
| TASK-404 | Database | Transactional Import | P0 | 2d | TASK-402 | Track و checkpoint اتمیک ذخیره می‌شوند |
| TASK-501 | Telegram | TDLib Build Integration | P0 | 5d | TASK-001 | TDLib C/JSON build می‌شود |
| TASK-502 | Telegram | TDLib Client Lifecycle | P0 | 3d | TASK-501 | Create/request/update/shutdown کار می‌کند |
| TASK-503 | Telegram | Telegram Authentication | P0 | 5d | TASK-502 | Login واقعی Telegram انجام می‌شود |
| TASK-504 | Telegram | Resolve Saved Messages | P0 | 2d | TASK-503 | Saved Messages قابل شناسایی است |
| TASK-505 | Telegram | Discover Channels و Bot Chats | P0 | 4d | TASK-504 | منابع قابل کشف هستند |
| TASK-601 | Sources | Source Selection Backend | P0 | 3d | TASK-505 | انتخاب/حذف source ذخیره می‌شود |
| TASK-602 | Sources UI | Music Sources Screen | P0 | 4d | TASK-601 | مطابق `concept/music-sources.png` پیاده شده |
| TASK-603 | Sources | Source Status Screen | P1 | 3d | TASK-602 | وضعیت sync نمایش داده می‌شود |
| TASK-701 | Indexing | Telegram Message Classifier | P0 | 4d | TASK-505 | Audio/Document/Voice/URL درست تشخیص داده می‌شوند |
| TASK-702 | Indexing | Metadata Extraction | P0 | 3d | TASK-701 | title/artist/duration استخراج می‌شود |
| TASK-703 | Indexing | Incremental Sync Engine | P0 | 7d | TASK-702 | checkpoint، resume، deduplication فعال است |
| TASK-704 | Indexing | Sync Test Suite | P1 | 3d | TASK-703 | crash/retry/duplicate تست شده |
| TASK-801 | Search | Search Normalization Engine | P0 | 3d | TASK-402 | Persian ی/ک normalization کار می‌کند |
| TASK-802 | Search | Search Repository Query | P0 | 2d | TASK-801 | title/artist/file search فعال است |
| TASK-803 | Search UI | Search Screen | P0 | 4d | TASK-802 | مطابق `concept/search.png` ساخته شده |
| TASK-901 | Library | Favorites System | P0 | 2d | TASK-402 | Favorite persist می‌شود |
| TASK-902 | Library | Playlist CRUD | P0 | 4d | TASK-402 | create/update/delete playlist |
| TASK-903 | Library | Playlist Ordering | P1 | 2d | TASK-902 | reorder اتمیک است |
| TASK-904 | Library UI | Library Screen | P0 | 5d | TASK-901 | مطابق `concept/library.png` ساخته شده |
| TASK-1001 | Home | Home Data Queries | P1 | 3d | TASK-402 | Recently Added و Continue Listening آماده است |
| TASK-1002 | Home UI | Home Screen | P0 | 5d | TASK-1001 | مطابق `concept/home.png` ساخته شده |
| TASK-1101 | Downloads | Download Manager | P0 | 5d | TASK-302 | queue/download state پیاده شده |
| TASK-1102 | Downloads | Local File Validation | P0 | 2d | TASK-1101 | فقط فایل کامل offline-ready می‌شود |
| TASK-1103 | Downloads UI | Offline Library | P1 | 3d | TASK-1101 | فایل‌های offline نمایش داده می‌شوند |
| TASK-1201 | Playback | Playback Controller | P0 | 5d | TASK-302 | queue/state در Rust کنترل می‌شود |
| TASK-1202 | Playback | Queue Management | P0 | 3d | TASK-1201 | add/remove/reorder/next کار می‌کند |
| TASK-1203 | Playback | Playback Snapshot Persistence | P1 | 2d | TASK-1201 | state قابل restore است |
| TASK-1301 | Android Media | Media3 Adapter | P0 | 7d | TASK-1201 | play/pause/seek اجرا می‌شود |
| TASK-1302 | Android Media | MediaSessionService | P0 | 5d | TASK-1301 | background playback فعال است |
| TASK-1303 | Android Media | Audio Focus Handling | P1 | 3d | TASK-1302 | call/headset interruption مدیریت می‌شود |
| TASK-1401 | UI System | Design System Implementation | P0 | 5d | TASK-001 | tokenها و componentها ساخته شده‌اند |
| TASK-1402 | UI | Login Screen | P0 | 3d | TASK-1401 | مطابق `concept/login.png` |
| TASK-1403 | UI | Mini Player | P0 | 3d | TASK-1201 | مطابق `concept/mini-player.png` |
| TASK-1404 | UI | Main Player | P0 | 5d | TASK-1201 | مطابق `concept/main-player.png` |
| TASK-1405 | UI | Car Mode | P1 | 4d | TASK-1201 | مطابق `concept/car-mode.png` |
| TASK-1501 | Error Handling | Typed Error Model | P0 | 2d | TASK-201 | خطاها structured هستند |
| TASK-1502 | Error Handling | Recovery Flows | P1 | 4d | TASK-1501 | retry/reconnect فعال است |
| TASK-1601 | Security | Secure Storage | P0 | 3d | TASK-503 | session data امن ذخیره می‌شود |
| TASK-1602 | Security | Logging Policy | P0 | 1d | TASK-1601 | credentialها log نمی‌شوند |
| TASK-1701 | Account | Logout Workflow | P0 | 3d | TASK-503 | logout کامل و امن است |
| TASK-1801 | Testing | Core Unit Tests | P0 | 4d | TASK-101 | domain/application تست دارند |
| TASK-1802 | Testing | Database Tests | P0 | 3d | TASK-401 | migration و transaction تست شده |
| TASK-1803 | Testing | Telegram Mock Tests | P1 | 3d | TASK-505 | indexing scenarioها تست شده |
| TASK-1804 | Testing | Playback State Tests | P0 | 3d | TASK-1201 | state machine تست شده |
| TASK-1805 | Testing | UI Screenshot Tests | P1 | 4d | TASK-1401 | مقایسه با conceptها انجام می‌شود |
| TASK-1901 | Documentation | Developer Setup Docs | P1 | 2d | - | نصب و build مستند شده |
| TASK-1902 | Documentation | Architecture Docs | P1 | 3d | TASK-301 | runtime و boundaryها مستند هستند |
| TASK-1903 | Documentation | Implementation Status | P0 | 1d | - | وضعیت واقعی پروژه ثبت می‌شود |
| TASK-2001 | Release | G0 Foundation Verification | P0 | 1d | All Foundation | build evidence ثبت شده |
| TASK-2002 | Release | G1 Persistence Verification | P0 | 2d | Database | persistence gate پاس شده |
| TASK-2003 | Release | G2 Media Verification | P0 | 3d | Android Media | تست واقعی device انجام شده |
| TASK-2004 | Release | G3 Telegram Verification | P0 | 3d | Telegram | login/import/download واقعی |
| TASK-2005 | Release | G4 Alpha Verification | P0 | 3d | All | AC-01 تا AC-09 تایید شده |
