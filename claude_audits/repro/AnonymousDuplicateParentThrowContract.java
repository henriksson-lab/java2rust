interface LeftCheck {
    int value(boolean fail);
}

interface RightCheck {
    int value(boolean fail);
}

interface BothCheck extends LeftCheck, RightCheck {}

class DuplicateThrowUse {
    static BothCheck make() {
        return new BothCheck() {
            int total = 3;

            public int value(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return this.total;
            }
        };
    }

    static int run() {
        return make().value(false);
    }

    static int runLeft() {
        LeftCheck left = make();
        return left.value(false);
    }

    static int runRight() {
        RightCheck right = make();
        return right.value(false);
    }

    static int runFail() {
        return make().value(true);
    }
}

public class AnonymousDuplicateParentThrowContract {
    public static void main(String[] args) {
        System.out.println(DuplicateThrowUse.run() + DuplicateThrowUse.runLeft()
                + DuplicateThrowUse.runRight());
        try {
            System.out.println(DuplicateThrowUse.runFail());
        } catch (IllegalStateException expected) {
            System.out.println("caught");
        }
    }
}
