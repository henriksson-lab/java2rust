class BlockBase {
    BlockBase() {}
    int value() { return 1; }
}

public class AnonymousClassLabeledBlock {
    static BlockBase make() {
        return new BlockBase() {
            int total = 0;

            @Override int value() {
                int i = 0;
                outer: {
                    int bonus = 3;
                    while (i < 4) {
                        i++;
                        {
                            int local = bonus + i;
                            this.total = this.total + local;
                        }
                        if (i == 2) break outer;
                    }
                    this.total = 100;
                }
                return this.total * 10 + i + super.value();
            }
        };
    }

    public static void main(String[] args) {
        System.out.println(make().value());
    }
}
