public class EnumCompareTo {
    enum Mode {
        ALPHA,
        BETA,
        GAMMA
    }

    public static String run() {
        return Mode.ALPHA.compareTo(Mode.BETA)
            + "," + Mode.BETA.compareTo(Mode.ALPHA)
            + "," + Mode.GAMMA.compareTo(Mode.GAMMA);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
