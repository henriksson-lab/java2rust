public class StringBoundsException {
    static String charAtBad() {
        try {
            return "" + "a".charAt(2);
        } catch (StringIndexOutOfBoundsException e) {
            return "char";
        }
    }

    static String substringBad() {
        try {
            return "abc".substring(2, 1);
        } catch (StringIndexOutOfBoundsException e) {
            return "sub";
        }
    }

    public static String run() {
        return charAtBad() + "," + substringBad();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
