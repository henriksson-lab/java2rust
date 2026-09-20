class EachArrayBase {
    EachArrayBase() {}
    int sum(int[] values) { return 0; }
}

public class AnonymousClassEnhancedFor {
    static EachArrayBase make() {
        return new EachArrayBase() {
            int total = 0;

            @Override int sum(int[] values) {
                outer: for (int value : values) {
                    if (value == 2) continue outer;
                    this.total = this.total + value;
                    if (value == 3) break outer;
                }
                return this.total;
            }
        };
    }

    public static void main(String[] args) {
        EachArrayBase value = make();
        int[] first = {1, 2, 3, 4};
        int[] second = {5};
        System.out.println(value.sum(first) + value.sum(second));
    }
}
