import java.awt.BorderLayout;
import java.awt.GridLayout;
import javax.swing.*;

/** Safe Swing fixture used only for cross-runtime accessibility probing. */
public final class SwingLiveFixture {
    public static void main(String[] args) {
        SwingUtilities.invokeLater(() -> {
            JFrame frame = new JFrame("GUI2TUI Swing Fixture");
            frame.setDefaultCloseOperation(WindowConstants.EXIT_ON_CLOSE);

            JMenu demo = new JMenu("Tools");
            JMenuItem menuAction = new JMenuItem("Activate Demo");
            demo.add(menuAction);
            JMenuBar menuBar = new JMenuBar();
            menuBar.add(demo);
            frame.setJMenuBar(menuBar);

            JPanel form = new JPanel(new GridLayout(0, 2, 8, 8));
            JTextField username = new JTextField("alice");
            JPasswordField password = new JPasswordField("swing-phase-secret");
            JCheckBox enabled = new JCheckBox("Enable feature");
            JLabel status = new JLabel("Status: idle");
            JButton activate = new JButton("Activate safely");
            JList<String> items = new JList<>(new String[] {"Alpha", "Beta", "Gamma"});
            form.add(new JLabel("Username"));
            form.add(username);
            form.add(new JLabel("Password"));
            form.add(password);
            form.add(enabled);
            form.add(status);
            form.add(activate);
            form.add(new JScrollPane(items));

            Runnable action = () -> {
                enabled.setSelected(true);
                status.setText("Status: activated");
            };
            activate.addActionListener(event -> action.run());
            menuAction.addActionListener(event -> action.run());
            frame.add(form, BorderLayout.CENTER);
            frame.pack();
            frame.setSize(560, 360);
            frame.setVisible(true);
        });
    }
}
