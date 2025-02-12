#!/usr/bin/env node

import CDP from 'chrome-remote-interface';
import { launch, LaunchedChrome } from 'chrome-launcher';
import { DebugSessionManager } from '../core/DebugSession'
import { CommandReader } from './CommandReader'
import { DebugAdapter } from '../core/DebugAdapterInterface';
import minimist from 'minimist';

class DummyDebugAdapter implements DebugAdapter {
    sendEvent() {
        // do nothing
    }
}

interface CommandOptions {
    page?: string;
}

async function main() {
    let client: CDP.Client | null = null;
    let launchedBrowser: LaunchedChrome | null = null;

    const argv = minimist(process.argv.slice(2), {
        alias: {
            p: 'page'
        }
    }) as CommandOptions;
    
    try {
        launchedBrowser = await launch({
        });

        // connect to endpoint
        client = await CDP({
            port: launchedBrowser.port
        });

        // extract domains
        const { Debugger, Page, Runtime, Console } = client;

        const manager = new DebugSessionManager(new DummyDebugAdapter());
        manager.setChromeDebuggerApi(Debugger, Page, Runtime);

        await Console.enable();
		Console.on("messageAdded", e => {
			if (e.message.level == "error") {
				console.error(e.message.text);
			} else {
				console.log(e.message.text);
			}
		});
      
        await Debugger.enable({});
        await Debugger.setInstrumentationBreakpoint({ instrumentation: "beforeScriptExecution" });
        await Runtime.enable();
        await Runtime.runIfWaitingForDebugger();
        await Page.enable();

        const commandReader = new CommandReader(manager);

        if (argv.page) {
            await commandReader.jumpToPage(argv.page);
        }

        await commandReader.start();
        
    } catch (err) {
        console.error(err);
    } finally {
        if (client) {
            console.log('session closed.');
            void client.close();
        }

        if (launchedBrowser) {
            await launchedBrowser.kill();
        }
    }
}

void main();
