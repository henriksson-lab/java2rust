public class CharacterStatics {
    public static String run() {
        return Character.isDigit('7') + "," + Character.isLetter('a') + "," + Character.toUpperCase('q') + "," + Character.toLowerCase('Q') + "," + Character.isWhitespace(' ');
    }
    public static void main(String[] args) { System.out.println(run()); }
}
