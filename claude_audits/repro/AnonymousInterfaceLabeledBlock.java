interface BlockCounter { int advance(); int read(); }

public class AnonymousInterfaceLabeledBlock {
    public static void main(String[] args) {
        BlockCounter counter = new BlockCounter() {
            int total = 0;

            public int advance() {
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
                return this.total * 10 + i;
            }

            public int read() { return this.total; }
        };
        System.out.println(counter.advance() * 100 + counter.read());
    }
}
