public class StringBuilderDeleteInsertCatch {
    public static String run() {
        StringBuilder builder = new StringBuilder("abcd");
        builder.delete(1, 3).insert(1, "XY");
        String value = builder.toString();
        String deleteStatus;
        try {
            builder.delete(5, 4);
            deleteStatus = "miss";
        } catch (StringIndexOutOfBoundsException e) {
            deleteStatus = "delete";
        }
        String insertStatus;
        try {
            builder.insert(-1, "!");
            insertStatus = "miss";
        } catch (StringIndexOutOfBoundsException e) {
            insertStatus = "insert";
        }
        return value + ":" + deleteStatus + ":" + insertStatus;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
