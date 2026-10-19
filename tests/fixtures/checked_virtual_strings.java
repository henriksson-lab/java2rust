import java.util.*;
import java.util.stream.*;

class StreamText {
    int mode;
    StreamText(int value) { mode = value; }
    public String toString() {
        if (mode == 1) throw new IllegalStateException();
        if (mode == 2) return null;
        return "ok";
    }
}
class GenericTextStream<T> {
    String join(List<T> values) {
        return values.stream().map(value -> value.toString()).collect(Collectors.joining(","));
    }
    String reference(List<T> values) {
        return values.stream().map(T::toString).collect(Collectors.joining(","));
    }
    static <U> U checked(U value) {
        if (value == null) throw new IllegalArgumentException();
        return value;
    }
    String afterCall(List<T> values) {
        return values.stream().map(value -> checked(value).toString()).collect(Collectors.joining(","));
    }
}
class CheckedVirtualStrings {
    static List<StreamText> values(int mode) {
        List<StreamText> result = new ArrayList<StreamText>();
        result.add(new StreamText(0));
        if (mode == 3) result.add(null);
        else result.add(new StreamText(mode));
        return result;
    }
    static String direct(int mode) {
        GenericTextStream<StreamText> renderer = new GenericTextStream<StreamText>();
        try { return renderer.join(values(mode)); }
        catch (IllegalStateException ex) { return "caught"; }
        catch (NullPointerException ex) { return "null receiver"; }
    }
    static String afterCall(int mode) {
        GenericTextStream<StreamText> renderer = new GenericTextStream<StreamText>();
        try { return renderer.afterCall(values(mode)); }
        catch (IllegalStateException ex) { return "caught"; }
        catch (IllegalArgumentException ex) { return "bad argument"; }
    }
    static String reference(int mode) {
        GenericTextStream<StreamText> renderer = new GenericTextStream<StreamText>();
        try { return renderer.reference(values(mode)); }
        catch (IllegalStateException ex) { return "caught"; }
        catch (NullPointerException ex) { return "null receiver"; }
    }
}
