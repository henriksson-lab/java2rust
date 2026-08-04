public class TryMove {
    private String held = "";
    void take(String s) { held = s; }
    public String run(String value) {
        try { take(value); return "ok"; }
        catch (RuntimeException e) { return value; }
    }
}
