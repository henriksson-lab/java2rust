package examples.control;

public class ForOmittedParts {
    public int countWithEmptyFor() {
        int i = 0;
        for (;;) {
            if (i > 3) {
                break;
            }
            i++;
        }
        return i;
    }

    public int countWithoutCondition() {
        int sum = 0;
        for (int i = 0; ; i++) {
            if (i > 3) {
                break;
            }
            sum += i;
        }
        return sum;
    }

    public int countWithOnlyUpdate() {
        int i = 0;
        for (;; i++) {
            if (i > 3) {
                break;
            }
        }
        return i;
    }
}
