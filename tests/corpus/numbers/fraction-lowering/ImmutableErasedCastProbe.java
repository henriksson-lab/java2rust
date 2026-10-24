package immutableerasedcast;

/** A successful instanceof and its cast must use the same runtime carrier. */
public final class ImmutableErasedCastProbe {
    static final class LocalValue {
        final int number;
        LocalValue(int number) { this.number=number; }
        @Override public boolean equals(Object other) {
            if (other==this) return true;
            if (other instanceof LocalValue) {
                final LocalValue value=(LocalValue)other;
                return number==value.number;
            }
            return false;
        }
    }
    static final class GlobalValue {
        static final GlobalValue ZERO=new GlobalValue(0);
        final int number;
        GlobalValue(int number) { this.number=number; }
        @Override public boolean equals(Object other) {
            if (other==this) return true;
            if (other instanceof GlobalValue) {
                final GlobalValue value=(GlobalValue)other;
                return number==value.number;
            }
            return false;
        }
    }
    public static int run() {
        LocalValue local=new LocalValue(7);
        Object equalLocal=new LocalValue(7);
        Object differentLocal=new LocalValue(8);
        if (!local.equals(equalLocal) || local.equals(differentLocal)) return 1;
        if (!local.equals(local) || local.equals(null) || local.equals("other")) return 2;
        GlobalValue global=new GlobalValue(7);
        Object equalGlobal=new GlobalValue(7);
        Object differentGlobal=new GlobalValue(8);
        if (!global.equals(equalGlobal) || global.equals(differentGlobal)) return 3;
        if (!global.equals(global) || global.equals(null) || global.equals(local)) return 4;
        if (!GlobalValue.ZERO.equals(new GlobalValue(0))) return 5;
        if (!local.equals(equalLocal) || !global.equals(equalGlobal)) return 6;
        return 0;
    }
    public static void main(String[] args) {
        int result=run();
        if (result!=0) throw new AssertionError("immutable erased cast: "+result);
        System.out.println(result);
    }
}
