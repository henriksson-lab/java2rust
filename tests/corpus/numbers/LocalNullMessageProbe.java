public final class LocalNullMessageProbe {
    static int basic(int[] input) {
        int[] local = input;
        return local.length;
    }
    int instance(int[] input) {
        int[] local = input;
        return local.length;
    }
    static int wide(double weight, long count, int[] input) {
        int[] local = input;
        return local.length;
    }
    static int reuse(int[] input) {
        { long padding = 3; if (padding == 0) return 0; }
        int[] local = input;
        return local.length;
    }
    static int loop(int[][] rows) {
        int total = rows.length;
        for (int i=0; i<total; i++) {
            int[] row = rows[i];
            return row.length;
        }
        return 0;
    }
    static int shadow(int[] input) {
        { int[] local = {1}; if (local.length == 0) return 0; }
        { long padding = 1; int[] local = input; if (padding > 0) return local.length; }
        return 0;
    }
    static int constant(int[] input) {
        final double padding = 1;
        int[] local = input;
        return local.length;
    }
    static int enhanced(int[][] rows) {
        for (int[] row : rows) return row.length;
        return 0;
    }
    static int string(String input) {
        String local = input;
        return local.length();
    }
    static final class Box { int value; }
    static int field(Box input) {
        Box local = input;
        return local.value;
    }
    public static int run() {
        int[] slots = {1,2,5,1,3,3,1,1,1,1,4};
        for (int which=0; which<slots.length; which++) {
            try {
                switch (which) {
                    case 0: basic(null); break;
                    case 1: new LocalNullMessageProbe().instance(null); break;
                    case 2: wide(2.0,3L,null); break;
                    case 3: reuse(null); break;
                    case 4: loop(new int[1][]); break;
                    case 5: shadow(null); break;
                    case 6: constant(null); break;
                    case 7: string(null); break;
                    case 8: field(null); break;
                    case 9: enhanced(null); break;
                    default: enhanced(new int[1][]);
                }
                return 100+which;
            } catch (NullPointerException error) {
                String operation = which == 7 ? "invoke \"String.length()\"" : which == 8 ? "read field \"value\"" : "read the array length";
                String expected = "Cannot "+operation+" because \"<local"+slots[which]+">\" is null";
                if (!expected.equals(error.getMessage())) {
                    System.err.println(which+": "+error.getMessage());
                    return 1+which;
                }
            }
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
