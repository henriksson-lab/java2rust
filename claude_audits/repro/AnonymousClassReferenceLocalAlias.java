class AliasNode {
    int value;

    AliasNode(int value) {
        this.value = value;
    }

    int read() {
        return this.value;
    }
}

class AliasBase {
    AliasBase() {}

    int step(int delta) {
        return -1;
    }

    int bonus() {
        return 2;
    }
}

class AliasUse {
    static AliasBase make(AliasNode original) {
        return new AliasBase() {
            AliasNode node = original;

            @Override int step(int delta) {
                AliasNode local = this.node;
                if (delta > 0) {
                    local.value = local.value + delta;
                }
                return this.node.read() + super.bonus();
            }
        };
    }

    static int run() {
        AliasNode original = new AliasNode(3);
        AliasBase value = make(original);
        return value.step(2) * 10 + original.read();
    }
}

public class AnonymousClassReferenceLocalAlias {
    public static void main(String[] args) {
        System.out.println(AliasUse.run());
    }
}
