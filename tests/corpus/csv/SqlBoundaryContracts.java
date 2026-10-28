import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;

public class SqlBoundaryContracts {
    public static String[] labels(ResultSetMetaData metadata) throws SQLException {
        if (metadata == null) return null;
        String[] labels = new String[metadata.getColumnCount()];
        for (int i = 0; i < labels.length; i++) labels[i] = metadata.getColumnLabel(i + 1);
        return labels;
    }
    public static int header(ResultSet resultSet) throws SQLException {
        String[] values = labels(resultSet == null ? null : resultSet.getMetaData());
        return values == null ? 7 : values.length;
    }
    public static int relay(ResultSet resultSet) throws SQLException { return header(resultSet); }
    public static int rows(ResultSet resultSet, boolean includeHeader) throws SQLException {
        if (resultSet == null) return relay(resultSet);
        if (includeHeader) relay(resultSet);
        int count = 0;
        while (resultSet.next()) count++;
        return count;
    }
    public static int run() throws SQLException {
        if (labels(null) != null || header(null) != 7) return 1;
        if (rows(null, true) != 7 || rows(null, false) != 7) return 2;
        return 0;
    }
    public static void main(String[] args) throws SQLException { System.out.println(run()); }
}
