import java.util.ArrayList;

public class TextAssemblyClient {
    public static int run() {
        String text = TextAssemblyValues.text();
        String copy = TextAssemblyValues.copied(text);
        if (!copy.equals(text) || copy == text) return 1;
        ArrayList<String> values = new ArrayList<>();
        values.add(text);
        String displayed = values.toString();
        if (displayed.length() != 5 || displayed.charAt(1) != '\ud800' || displayed.charAt(3) != '\udc00') return 2;
        Object erased = copy;
        String appended = new StringBuilder().append(erased).toString();
        if (!appended.equals(text)) return 3;
        if (!("[" + text + "]").equals(displayed)) return 4;
        if (!TextAssemblyValues.formatted(text).equals("pre:" + text + ":post")) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println("contract=" + run()); }
}
