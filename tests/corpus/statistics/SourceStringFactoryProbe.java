public final class SourceStringFactoryProbe {
    static final class Holder {
        static final class String {
            static java.lang.String valueOf(int value) {
                return value == 3 ? "source" : "wrong";
            }
        }
    }
    public static int run() {
        java.lang.String value = Holder.String.valueOf(3);
        return "source".equals(value) ? 0 : 1;
    }
    public static void main(java.lang.String[] args) { System.out.println(run()); }
}
