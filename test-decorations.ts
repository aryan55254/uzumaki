import { Window } from 'uzumaki';

async function main() {
  const win = new Window('test_win', {
    title: 'Testing Setters/Getters',
    width: 600,
    height: 400,
    visible: true,
    resizable: false,
    maximized: true,
    decorations: false,
  });

  console.log('Initial state:');
  console.log('  visible:', win.visible);
  console.log('  resizable:', win.resizable);
  console.log('  maximized:', win.maximized);
  console.log('  decorations:', win.decorations);

  // set runtime
  setInterval(() => {
    win.decorations = !win.decorations;
    win.resizable = true;
    console.log(
      'Toggled decorations! decorations is now',
      win.decorations,
      'resizable is now',
      win.resizable,
    );
  }, 2000);
}

try {
  await main();
} catch (error) {
  console.error(error);
}
