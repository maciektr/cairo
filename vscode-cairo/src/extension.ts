import * as vscode from 'vscode';
import {SemanticTokensFeature} from 'vscode-languageclient/lib/common/semanticTokens';
import * as child_process from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

async function findExecutableInPath(name: string) {
    const envPath = process.env.PATH || "";
    const envExt = process.env.PATHEXT || "";
    const pathDirs = envPath
        .replace(/["]+/g, "")
        .split(path.delimiter)
        .filter(Boolean);
    const extensions = envExt.split(";");
    let candidates: string[] = [];
    pathDirs.map((d) =>
        extensions.map((ext) => {
            candidates.push(path.join(d, name + ext));
        })
    );
    const fileExists = (path: string) => fs.promises.stat(path).then(
        (stat) => stat.isFile() ? path : undefined, () => undefined
    );
    try {
        return await Promise.all(candidates.map(fileExists)).then((values) =>
            values.find((value) => !!value)
        );
    } catch (e) {
        return undefined;
    }
}

function findExecutableByConfig(
    config: vscode.WorkspaceConfiguration, context: vscode.ExtensionContext,
    configurationKey: string,
    defaultCallback: (rootPath: string) => string | undefined
) {

    let rootPath = context.extensionPath;

    let workspaceFolders = vscode.workspace.workspaceFolders;
    if (workspaceFolders) {
        rootPath = workspaceFolders[0].uri.path || rootPath
    }

    var configPath = config.get<string>(configurationKey);
    if (configPath) {
        // Replace placeholders, if present.
        let serverPath = configPath.replace(/\${workspaceFolder}/g, rootPath);
        if (!fs.existsSync(serverPath)) {
            return undefined;
        }
        return serverPath;
    }

    return defaultCallback(rootPath);
}

// Tries to find the development version of the language server executable,
// assuming the workspace directory is inside the Cairo repository.
function findDevLanguageServerAt(path: string, depth: number): string | undefined {
    if (depth == 0) {
        return undefined;
    }
    let candidate = path + "/target/release/cairo-language-server";
    if (fs.existsSync(candidate)) {
        return candidate;
    }
    candidate = path + "/target/debug/cairo-language-server";
    if (fs.existsSync(candidate)) {
        return candidate;
    }
    return findDevLanguageServerAt(path + "/..", depth - 1);
}

function findLanguageServerExecutable(
    config: vscode.WorkspaceConfiguration, context: vscode.ExtensionContext) {

    return findExecutableByConfig(config, context, 'cairo1.languageServerPath',
        // TODO(spapini): Use a bundled language server.
        (rootPath: string) => findDevLanguageServerAt(rootPath, 10)
    );

}

async function findScarbExecutable(
    config: vscode.WorkspaceConfiguration, context: vscode.ExtensionContext) {

    const scarbFromPath = await findExecutableInPath("scarb");
    return findExecutableByConfig(config, context, 'cairo1.scarbPath',
        (_rootPath: string) => scarbFromPath);

}


function setupLanguageServer(
    config: vscode.WorkspaceConfiguration, context: vscode.ExtensionContext, outputChannel: vscode.OutputChannel) {

    let serverOptions: ServerOptions = () => {
        return new Promise((resolve) => {
            let executable = findLanguageServerExecutable(config, context);
            if (!executable) {
                outputChannel.appendLine(
                    "Cairo language server was not found. Make sure cairo-lang-server is " +
                    "installed and that the configuration 'cairo1.languageServerPath' is correct.");
                return;
            }
            outputChannel.appendLine("Cairo language server running from: " + executable);
            let child = child_process.spawn(executable);
            // Forward stderr to vscode logs.
            child.stderr.on('data', (data: Buffer) => {
                outputChannel.appendLine("Server stderr> " + data.toString());
            });
            child.on('exit', (code, signal) => {
                outputChannel.appendLine('Cairo language server exited with code ' + code +
                    ' and signal' + signal);
            });

            // Create a resolved promise with the child process.
            resolve(child);
        });
    }

    let clientOptions: LanguageClientOptions = {
        documentSelector: [
            {scheme: 'file', language: 'cairo'},
            {scheme: 'vfs', language: 'cairo'}],
    };

    var client: LanguageClient = new LanguageClient(
        'cairoLanguageServer', 'Cairo Language Server', serverOptions, clientOptions);
    client.registerFeature(new SemanticTokensFeature(client));
    client.onReady().then(() => {
        const myProvider = new (class implements vscode.TextDocumentContentProvider {
            onDidChangeEmitter = new vscode.EventEmitter<vscode.Uri>();
            onDidChange = this.onDidChangeEmitter.event;

            async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
                const res: any = await client.sendRequest("vfs/provide", {uri: uri.toString()});
                return res.content;
            }
        })();
        client.onNotification('vfs/update', (param) => {
            myProvider.onDidChangeEmitter.fire(param.uri)

        });
        vscode.workspace.registerTextDocumentContentProvider("vfs", myProvider);
    });
    client.start();
}

async function setupScarb(config: vscode.WorkspaceConfiguration, context: vscode.ExtensionContext, outputChannel: vscode.OutputChannel) {
    const path = await findScarbExecutable(config, context);
    if (!path) {
        const errorMessage = "Scarb executable not found! Please add Scarb to the PATH "
            + "environmental variable or set the 'cairo1.scarbPath' configuration parameter.";
        vscode.window.showWarningMessage(errorMessage);
        outputChannel.appendLine(errorMessage);
    } else {
        outputChannel.appendLine("Using Scarb binary from: " + path);
    }
}

export async function activate(context: vscode.ExtensionContext) {
    const config = vscode.workspace.getConfiguration();
    let outputChannel = vscode.window.createOutputChannel("Cairo extension");
    context.subscriptions.push(outputChannel);

    await setupScarb(config, context, outputChannel);

    if (config.get<boolean>('cairo1.enableLanguageServer')) {
        setupLanguageServer(config, context, outputChannel);
    } else {
        outputChannel.appendLine(
            "Language server is not enabled. Use the cairo1.enableLanguageServer config");
    }
}
