package probe.enums;
public class EnumMetadataPeer {
    static int initialized;
    public static Runnable callback;
    enum Peer {
        OTHER;
        Peer() { initialized++; if (callback != null) callback.run(); }
    }
    public static Class<?> token() { return Peer.class; }
    public static int initializedCount() { return initialized; }
    public static int run() {
        Class<?> value = token();
        if (initialized != 0) return 1;
        if (value != Peer.class) return 2;
        return 0;
    }
}
