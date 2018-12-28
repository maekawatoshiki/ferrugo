package java.lang;
import java.io.PrintStream;

public class System {
  public static final PrintStream out;
  static {
    out = new PrintStream();
  }
}
