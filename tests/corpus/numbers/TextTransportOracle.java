import java.io.*;
import java.nio.charset.StandardCharsets;

public class TextTransportOracle {
    static String units(CharSequence value) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < value.length(); i++) out.append(String.format("%04x", (int) value.charAt(i)));
        return out.toString();
    }
    static String bytes(ByteArrayOutputStream value) {
        StringBuilder out = new StringBuilder();
        for (byte b : value.toByteArray()) out.append(String.format("%02x", b & 255));
        return out.toString();
    }
    public static void main(String[] args) throws Exception {
        int[][] payloads = {{}, {65}, {233}, {0xd83d,0xde00}, {0xd800}, {0xdc00},
            {0xd800,0xd801}, {0xdc00,0xdc01}, {0xd800,65,0xdc00}, {0xdc00,0xd800}, {0}};
        for (int[] payload : payloads) {
            char[] raw = new char[payload.length];
            for (int i = 0; i < raw.length; i++) raw[i] = (char) payload[i];
            String text = new String(raw);
            StringBuilder builder = new StringBuilder(text);
            CharSequence sequence = builder;
            System.out.println(units(sequence));
            builder.append('\ud800');
            System.out.println(sequence.length() + ":" + Integer.toHexString(sequence.charAt(sequence.length()-1)));
            CharSequence slice = sequence.subSequence(0, text.length());
            builder.setCharAt(0, 'Z');
            System.out.println(units(slice));
            StringWriter strings = new StringWriter();
            Writer alias = strings;
            strings.write(text);
            alias.write(new char[]{'\ud800'});
            String snapshot = strings.toString();
            alias.write(new char[]{'\udc00'});
            System.out.println(units(snapshot));
            System.out.println(units(strings.toString()));
            alias.close();
            alias.write(new char[]{'\ud800'});
            System.out.println(units(strings.toString()));
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            Writer encoded = new OutputStreamWriter(bytes, StandardCharsets.UTF_8);
            encoded.write(text); encoded.flush();
            System.out.println(bytes(bytes));
            encoded.write(new char[]{'\ud800'}); encoded.flush();
            System.out.println(bytes(bytes));
            encoded.write(new char[]{'\udc00'}); encoded.flush();
            System.out.println(bytes(bytes));
            encoded.write(new char[]{'\ud800'}); encoded.close();
            System.out.println(bytes(bytes));
            try { encoded.write("x"); System.out.println("no-error"); }
            catch (IOException e) { System.out.println("IOException"); }
            ByteArrayOutputStream printed = new ByteArrayOutputStream();
            PrintStream printer = new PrintStream(printed, false, StandardCharsets.UTF_8);
            printer.print(text); printer.print('\ud800'); printer.flush();
            System.out.println(bytes(printed));
            printer.print('\udc00'); printer.flush();
            System.out.println(bytes(printed));
            printer.print('\ud800'); printer.close();
            System.out.println(bytes(printed));
        }
    }
}
