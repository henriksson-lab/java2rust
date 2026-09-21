import java.util.List;

public class NullableEnhancedForCatch {
    static class Node {
        int value;
    }

    static String arrayCase() {
        String[] values = null;
        try {
            for (String value : values) {
                return value;
            }
            return "empty";
        } catch (NullPointerException e) {
            return "array-npe";
        }
    }

    static String listCase() {
        List<String> values = null;
        try {
            for (String value : values) {
                return value;
            }
            return "empty";
        } catch (NullPointerException e) {
            return "list-npe";
        }
    }

    static String elementCase() {
        String[] values = {null, "x"};
        for (String value : values) {
            try {
                return "len=" + value.length();
            } catch (NullPointerException e) {
                return "element-npe";
            }
        }
        return "empty";
    }

    static String boxedCase() {
        Integer[] values = {null, 2};
        try {
            for (int value : values) {
                return "value=" + value;
            }
            return "empty";
        } catch (NullPointerException e) {
            return "boxed-npe";
        }
    }

    static String boxedValueCase() {
        Integer[] values = {2};
        for (int value : values) {
            return "value=" + value;
        }
        return "empty";
    }

    static String reassignedElementCase() {
        String[] values = {null};
        for (String value : values) {
            value = "replacement";
            return value;
        }
        return "empty";
    }

    static String referenceIdentityCase() {
        Node[] values = {new Node()};
        for (Node value : values) {
            value.value = 9;
        }
        return "shared=" + values[0].value;
    }

    static String readOnlyReferenceCase() {
        Node[] values = {new Node()};
        for (Node value : values) {
            return "read=" + value.value;
        }
        return "empty";
    }

    static String reboundReferenceCase() {
        Node[] values = {new Node()};
        for (Node value : values) {
            value = new Node();
            value.value = 5;
        }
        return "rebound=" + values[0].value;
    }

    static String externalAliasCase() {
        Node node = new Node();
        Node[] values = {node};
        for (Node value : values) {
            value.value = 11;
        }
        return "alias=" + node.value;
    }

    public static String run() {
        return arrayCase() + "," + listCase() + "," + elementCase() + ","
            + boxedCase() + "," + boxedValueCase() + "," + reassignedElementCase()
            + "," + referenceIdentityCase() + "," + readOnlyReferenceCase()
            + "," + reboundReferenceCase() + "," + externalAliasCase();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
