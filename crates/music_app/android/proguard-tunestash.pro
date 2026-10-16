# Rust calls these Kotlin methods by their exact JVM names. Dioxus's generated
# proguard-wry.pro already protects Wry and native entry points.
-keep class dev.dioxus.main.MainActivity {
    public static java.lang.String extractArtwork(java.lang.String);
    public static void playerCommand(android.content.Context, java.lang.String, java.lang.String);
    public static java.lang.String getOrCreateTdlibKey(android.content.Context);
    public static void clearTdlibKey(android.content.Context);
}
-keep class dev.dioxus.main.PlayerService {
    public static java.lang.String extractArtwork(java.lang.String);
    public static void playerCommand(android.content.Context, java.lang.String, java.lang.String);
    public static java.lang.String getOrCreateTdlibKey(android.content.Context);
    public static void clearTdlibKey(android.content.Context);
    native <methods>;
}
