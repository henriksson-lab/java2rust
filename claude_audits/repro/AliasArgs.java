public class AliasArgs {
    static String join(String a, String b) { return a + b; }
    public String run(String v) { return join(v, v); }
}
