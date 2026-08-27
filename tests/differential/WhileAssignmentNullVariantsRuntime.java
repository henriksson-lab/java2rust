public class WhileAssignmentNullVariantsRuntime {
    int drainUntilNull(Cursor cursor) {
        int count = 0;
        Row row;
        while (null != (row = cursor.read())) {
            count += row.size();
        }
        return count;
    }

    Row firstMissing(Cursor cursor) {
        Row row;
        while ((row = cursor.read()) == null) {
            cursor.advance();
        }
        return row;
    }

    public static String run() {
        WhileAssignmentNullVariantsRuntime probe = new WhileAssignmentNullVariantsRuntime();
        int drained = probe.drainUntilNull(new Cursor(0));
        Row first = probe.firstMissing(new Cursor(1));
        return drained + "," + first.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class Row {
    private final int n;

    Row(int n) {
        this.n = n;
    }

    int size() {
        return n;
    }
}

class Cursor {
    private final int mode;
    private int index = 0;

    Cursor(int mode) {
        this.mode = mode;
    }

    Row read() {
        if (mode == 0) {
            if (index == 0) {
                index++;
                return new Row(2);
            } else if (index == 1) {
                index++;
                return new Row(3);
            } else {
                return null;
            }
        } else if (index == 0) {
            return null;
        } else if (index == 1) {
            index++;
            return new Row(7);
        } else {
            return null;
        }
    }

    void advance() {
        index++;
    }
}
