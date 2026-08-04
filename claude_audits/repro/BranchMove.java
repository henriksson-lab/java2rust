public class BranchMove {
    private String held = "";
    void take(String s) { held = s; }
    public int run(String value, boolean flag) {
        if (flag) { take(value); return 1; }
        return value.length();
    }
}
