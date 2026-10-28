package probe.enums;
public class EnumLookupContracts {
    final String text;
    EnumLookupContracts(String text) { this.text = text; }
    static final EnumLookupContracts FIRST = new EnumLookupContracts("first");
    public enum Predefined {
        Default(FIRST);
        private final EnumLookupContracts format;
        Predefined(EnumLookupContracts format) { this.format = format; }
        public EnumLookupContracts getFormat() { return format; }
    }
    public static EnumLookupContracts valueOf(final String format) {
        return EnumLookupContracts.Predefined.valueOf(format).getFormat();
    }
    public static int run() {
        if (EnumMetadataPeer.run() != 0) return 4;
        if (valueOf("Default") != FIRST) return 1;
        try { valueOf("missing"); return 2; } catch (IllegalArgumentException expected) {}
        try { valueOf(null); return 3; } catch (NullPointerException expected) {}
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
