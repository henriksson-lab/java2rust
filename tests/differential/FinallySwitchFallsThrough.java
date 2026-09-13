public class FinallySwitchFallsThrough {
    static String log = "";

    static String runCase(int x) {
        log = "";
        try {
            switch (x) {
                case 0:
                    log = log + "b";
                    break;
                default:
                    return log + "r";
            }
        } finally {
            log = log + "f";
        }
        return log + "a";
    }

    public static String run() {
        return runCase(0) + ":" + runCase(1);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
