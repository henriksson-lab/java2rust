import java.io.StreamTokenizer;
import java.io.StringReader;
class StreamTokenizerTextProbe {
    static int run() throws Exception {
        StreamTokenizer tokenizer = new StreamTokenizer(new StringReader("alpha ; alpha"));
        if (tokenizer.sval != null) return 1;
        if (tokenizer.nextToken() != StreamTokenizer.TT_WORD) return 2;
        String first = tokenizer.sval;
        if (!"alpha".equals(first) || first != tokenizer.sval) return 3;
        tokenizer.pushBack();
        if (tokenizer.nextToken() != StreamTokenizer.TT_WORD || tokenizer.sval != first) return 4;
        StreamTokenizer alias = tokenizer;
        if (alias.nextToken() != ';' || tokenizer.sval != null) return 5;
        if (tokenizer.nextToken() != StreamTokenizer.TT_WORD) return 6;
        if (!tokenizer.sval.equals(first) || tokenizer.sval == first) return 7;
        if (tokenizer.nextToken() != StreamTokenizer.TT_EOF || tokenizer.sval != null) return 8;
        StreamTokenizer numeric = new StreamTokenizer(new StringReader("2.5 bad"));
        numeric.resetSyntax();
        numeric.wordChars('0', '9');
        numeric.wordChars('.', '.');
        numeric.wordChars('a', 'z');
        numeric.whitespaceChars(' ', ' ');
        try { Double.parseDouble(numeric.sval); return 9; }
        catch (NullPointerException expected) { }
        if (numeric.nextToken() != StreamTokenizer.TT_WORD) return 10;
        if (Double.parseDouble(numeric.sval) != 2.5 || Float.valueOf(numeric.sval) != 2.5f) return 11;
        if (numeric.nextToken() != StreamTokenizer.TT_WORD) return 12;
        try { Double.parseDouble(numeric.sval); return 13; }
        catch (NumberFormatException expected) { }
        return 0;
    }
    public static void main(String[] args) throws Exception { System.out.println(run()); }
}
