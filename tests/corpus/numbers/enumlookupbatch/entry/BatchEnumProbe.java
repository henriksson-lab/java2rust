package entry;
public class BatchEnumProbe {
    public static int run() {
        if (a.b.C.valueOf("VALUE") != a.b.C.VALUE || x.C.valueOf("VALUE") != x.C.VALUE) return 1;
        try { a.b.C.valueOf("missing"); return 2; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("No enum constant a.b.C.missing")) return 3;
        }
        try { x.C.valueOf("missing"); return 4; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("No enum constant x.C.missing")) return 5;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
