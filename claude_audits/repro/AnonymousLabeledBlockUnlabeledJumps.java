interface BlockJump { int run(); }
class BlockJumpBase { BlockJumpBase() {} int run() { return 0; } }

public class AnonymousLabeledBlockUnlabeledJumps {
    static BlockJumpBase make() {
        return new BlockJumpBase() {
            int total = 0;

            @Override int run() {
                int i = 0;
                while (i < 4) {
                    i++;
                    block: {
                        if (i == 2) continue;
                        if (i == 3) break;
                        this.total = this.total + i;
                    }
                }
                return this.total * 10 + i;
            }
        };
    }

    public static void main(String[] args) {
        BlockJump value = new BlockJump() {
            int total = 0;

            public int run() {
                int i = 0;
                while (i < 4) {
                    i++;
                    block: {
                        if (i == 2) continue;
                        if (i == 3) break;
                        this.total = this.total + i;
                    }
                }
                return this.total * 10 + i;
            }
        };
        System.out.println(value.run() * 100 + make().run());
    }
}
