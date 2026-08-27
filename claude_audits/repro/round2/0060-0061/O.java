public class O {
    int n;
    void mutate() { n = n + 1; }

    public void viaCatch() {
        try {
            throw new RuntimeException("x");
        } catch (RuntimeException e) {
            mutate();
        }
    }

    public void viaPlain() {
        mutate();
    }
}
