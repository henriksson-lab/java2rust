public class SelfRecurse {
    private int count = 0;
    public void walk(int depth) {
        count++;
        if (depth > 0) { walk(depth - 1); }
    }
}
