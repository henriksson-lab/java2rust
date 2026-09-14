public class SuperMutatingCallLabeledLoopControl {
    public static String run() {
        Derived d = new Derived();
        d.addWithLabeledBreak(4);
        d.addWithLabeledContinue(3);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseLabeledLoopControl {
    int n;

    void addWithLabeledBreak(int value) {
        outer:
        while (value > 0) {
            while (true) {
                if (value == 2) {
                    break outer;
                }
                this.n++;
                value--;
                break;
            }
        }
    }

    void addWithLabeledContinue(int value) {
        outer:
        while (value > 0) {
            value--;
            while (true) {
                if (value == 1) {
                    continue outer;
                }
                this.n++;
                break;
            }
        }
    }
}

class Derived extends BaseLabeledLoopControl {
    void addWithLabeledBreak(int value) {
        super.addWithLabeledBreak(value);
        this.n += 10;
    }

    void addWithLabeledContinue(int value) {
        super.addWithLabeledContinue(value);
        this.n += 10;
    }
}
