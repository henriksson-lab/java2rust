class LabeledWhileBase {
    LabeledWhileBase() {}
    int run(int limit) { return 0; }
    int runFor(int limit) { return 0; }
    int runDo(int limit) { return 0; }
    int runNested(int limit) { return 0; }
}

public class AnonymousClassLabeledWhile {
    static LabeledWhileBase make() {
        return new LabeledWhileBase() {
            int total = 0;

            @Override int run(int limit) {
                int i = 0;
                outer: while (i < limit) {
                    i++;
                    if (i == 2) continue outer;
                    this.total = this.total + i;
                    if (i == 3) break outer;
                }
                return this.total;
            }

            @Override int runFor(int limit) {
                int i = 0;
                outer: for (i = 0; i < limit; i++) {
                    if (i == 1) continue outer;
                    this.total = this.total + i;
                    if (i == 3) break outer;
                }
                return this.total + i;
            }

            @Override int runDo(int limit) {
                int i = 0;
                outer: do {
                    i++;
                    if (i == 2) continue outer;
                    this.total = this.total + i;
                    if (i == 3) break outer;
                } while (i < limit);
                return this.total + i;
            }

            @Override int runNested(int limit) {
                int i = 0;
                outer: for (i = 0; i < limit; i++) {
                    int inner = 0;
                    while (inner < 3) {
                        inner++;
                        if (inner == 2) continue outer;
                        this.total = this.total + 1;
                    }
                }
                return this.total + i;
            }
        };
    }

    public static void main(String[] args) {
        LabeledWhileBase value = make();
        System.out.println(value.run(5) + value.run(1));
        value = make();
        System.out.println(value.runFor(5) + value.runFor(2));
        value = make();
        System.out.println(value.runDo(5) + value.runDo(1));
        value = make();
        System.out.println(value.runNested(3) + value.runNested(1));
    }
}
