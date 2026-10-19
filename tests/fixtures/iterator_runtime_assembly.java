import java.util.*;
class CursorUse {
    static int run() {
        Iterator<String> cursor = Arrays.asList("a", "bb").iterator();
        return cursor.next().length() + cursor.next().length();
    }
}
