package textconstants;

import static textconstants.Constants.SUFFIX;

public class ConstantProbe {
    static final String LOCAL = "local";
    static String relay(String value) { return value; }
    public static int run() {
        if (relay(LOCAL) != "local") return 1;
        if (relay(Constants.PREFIX) != "pre:") return 2;
        if (relay(SUFFIX) != ":é😀") return 3;
        if (!(Constants.PREFIX + 3 + SUFFIX).equals("pre:3:é😀")) return 4;
        if (!String.format(Constants.PATTERN, LOCAL).equals("local")) return 5;
        if (!Constants.PREFIX.substring(0, 3).equals("pre")) return 6;
        String LOCAL = "shadow";
        if (relay(LOCAL) != "shadow") return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
