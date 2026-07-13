package examples.objects;

public class StaticFinalFields {
    public static final int LIMIT = 5;
    private String name;
    private boolean enabled = true;

    public int limit() {
        return LIMIT;
    }
}
