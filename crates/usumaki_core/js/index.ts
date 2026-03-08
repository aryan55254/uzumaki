import { Application, createWindow } from "./bindings"

export interface WindowAttributes {
  width: number,
  height: number
  title: string
}

export class Window {
  private _width: number
  private _height: number
  private _label: string

  constructor(label: string,  { width = 800, height = 600, title  = "Usumaki"}: Partial<WindowAttributes> = {}) {
    this._width = width;
    this._height = height;
    this._label = label;
    createWindow({width, height, label, title })
  }

  close() { }

  setSize(width: number, height: number) {
    this._width = width;
    this._height = height;
  }

  get width(): number {
    return this._width;
  }

  get height(): number {
    return this._height;
  }

  get label(): string {
    return this._label;
  }
}


export function runApp({ entryFilePath, title = "Usumaki" }: { entryFilePath: string, title?: string }) {
  let app = new Application();

  process.on("SIGINT", () => { });
  process.on("SIGTERM", () => { });

  console.log(entryFilePath);

  app.onInit(() => {
    new Worker(new URL("./main.ts", import.meta.url), {
      env: { ...process.env, entryPoint: entryFilePath }
    })
  })

  app.onWindowEvent(() => {
    // console.log("window event")
  })

  app.run()

  console.log("Reach here")
}
